# Como o app foi construído

Diário da construção, em ordem. Cada entrada diz o que foi feito, por que e o que se aprendeu de Rust ou de Windows no caminho.

## 06/11/2026: escolha da base e prova da janela

**Por que Rust e egui.** Bruno pediu Rust. Entre as bibliotecas de interface em Rust, a escolha foi o egui 0.37 (por meio do `eframe`). Ele desenha tudo em código, roda num processo só e não traz navegador embutido. As alternativas eram o Tauri 2.12.5, com interface em React dentro do WebView2 (visual mais livre e mais memória), e o Slint 1.18.1, com linguagem própria de interface. Versões conferidas no crates.io no mesmo dia.

**O que a prova precisava mostrar.** O ponto difícil não é desenhar o anel, e sim a janela:

1. transparente, para só o trilho dos anéis aparecer;
2. sempre por cima das outras janelas e presa na borda direita;
3. com o clique passando para a janela de trás fora do trilho;
4. com um cartão de detalhes quando o mouse passa sobre os anéis.

**Como cada item foi resolvido.**

- *Transparência.* `ViewportBuilder::with_transparent(true)` e `clear_color` devolvendo `[0.0; 4]`, que é a cor de fundo com alfa zero.
- *Por cima e sem moldura.* `with_always_on_top()`, `with_decorations(false)` e `with_taskbar(false)`. A posição é calculada no primeiro quadro, quando o egui já sabe o tamanho do monitor.
- *Clique passando.* `ViewportCommand::MousePassthrough(true)` faz o Windows ignorar a janela. O problema é que, assim, a janela também não sabe onde está o mouse. A saída foi perguntar a posição direto ao Windows (`GetCursorPos`, pela crate `windows-sys`) a cada 100 ms e desligar a passagem quando o cursor entra no trilho.
- *Cartão no hover.* Com a passagem desligada sobre o trilho, o cartão é só um retângulo e textos desenhados à esquerda.

**O tropeço: a moldura do Windows 11.** Mesmo sem moldura, o Windows 11 desenhou borda, cantos arredondados, sombra e uma linha cinza de 1 pixel no topo. Três ajustes resolveram:

- `DwmSetWindowAttribute` para tirar a sombra, os cantos e a cor da borda;
- tirar da janela os estilos `WS_CAPTION`, `WS_THICKFRAME` e `WS_SYSMENU`, que o winit (a camada de janelas usada pelo egui) deixa ligados;
- reaplicar tudo isso a cada troca da passagem do clique, porque o winit recalcula os estilos nesse momento e a moldura volta.

A linha de 1 pixel só sumiu com o terceiro ajuste. Antes disso, ela parecia um defeito do desenho, e era o winit devolvendo a moldura.

**Medido na prova** (PC de casa, Windows 11, tela 1920x1080):

- processador: entre 0,05% e 0,15% da máquina, com o redesenho a cada 100 ms;
- memória: cerca de 100 MB privados. É mais do que o esperado para um app tão pequeno e parece vir do driver de OpenGL (backend `glow`). Vale medir o backend `wgpu` antes de dar o app por leve;
- executável: 5,4 MB.

**Conferido por:** captura da borda da tela com e sem o app, e leitura dos estilos da janela (`GWL_EXSTYLE` com `WS_EX_TRANSPARENT` ligado fora do trilho e desligado sobre ele). Não testado: monitor com escala diferente de 100% e mais de um monitor.

**De onde virão os números.** Ainda não lê cota. O Codex grava a cota nos próprios arquivos de sessão (`~/.codex/sessions`), e o widget de limites do Brunarias Dock já lê dali. O Claude é o problema em aberto: neste PC não há login válido do CLI, e o statusline nunca entregou o número pelo app desktop.

## 07/11/2026: a cota do Claude

**O problema.** Bruno usa o Claude Code pelo app desktop. O app entrega o login ao `claude` pelo ambiente e o renova ele mesmo, então não fica arquivo de login nem roda o statusline. Não havia de onde ler.

**Como os outros fazem.** Seis monitores olhados pelo README (CodexBar, Usage Monitor for Claude, ClaudeMeter, claudeusagewin, claude-usage-tray, Pulse). Quase todos usam o login do Claude Code de terminal e resolvem o vencimento rodando o próprio `claude`. O cookie do app desktop aparece só como reserva. Bruno escolheu o caminho do login do terminal.

**O que foi feito.**

- Instalado o Claude Code 2.1.319 de terminal no Windows, pelo instalador oficial, em `%USERPROFILE%\.local\bin\claude.exe`, fora do PATH. Bruno fez `claude auth login` uma vez.
- `src/claude.rs` lê `%USERPROFILE%\.claude\.credentials.json`, usa o token se faltar mais de 1 minuto para vencer e chama `https://api.anthropic.com/api/oauth/usage` com o cabeçalho `anthropic-beta: oauth-2025-04-20`. O endereço não é documentado e pode mudar.
- Se o token venceu, ou se a resposta for 401, o app roda `claude update` escondido (`CREATE_NO_WINDOW`) e tenta de novo uma vez. O CLI renova o token nesse processo. A ideia veio do Usage Monitor for Claude.
- O app tira do processo filho as variáveis `CLAUDE*`, porque, rodando de dentro de uma sessão do app desktop, elas mudam o jeito do CLI tratar o login.
- A resposta tem dois formatos: o novo, com a lista `limits` (`session`, `weekly_all`, `weekly_scoped`), e o antigo, com `five_hour` e `seven_day`. Os dois são lidos.
- Uma linha de execução separada lê a cota a cada 3 minutos e pede o redesenho. O cartão mostra o percentual e a hora local da renovação, com o `chrono`.
- `aneis-de-cota --claude` lê uma vez e imprime no terminal, sem abrir a janela.

**Testes.** Seis testes em `src/claude.rs`, com dados inventados: token válido, vencido, quase vencendo, ausente ou quebrado, e os dois formatos de resposta. Rodados primeiro contra funções vazias. Três falharam pelo motivo esperado (RED), e os três de recusa passaram porque a função vazia já recusa tudo. Depois da implementação, os seis passaram (GREEN).

**Conferido no PC de casa.**

- Leitura real: 26% usado na janela de 5 horas e 21% na de 7 dias.
- Renovação: com a data do login trocada à mão para 5 minutos no passado, o app rodou `claude update`, o token mudou, a validade passou para 08:06 e a cota foi lida, em 3,6 segundos.
- Os anéis mostraram os números reais, e o cartão, a hora da renovação.

**Ponto de atenção.** A memória privada subiu de cerca de 100 MB para 180 MB depois da leitura pela rede. Ainda não foi investigado se vem do TLS (`rustls`), da linha de execução ou de outra coisa.

## 07/11/2026: a memória

**A dúvida.** Depois da leitura pela rede, o app parecia ter passado de cerca de 100 MB para 180 MB.

**O que foi medido.** A coluna "Memória" do Gerenciador de Tarefas é o conjunto de trabalho privado, a parte da memória do app que está de fato na RAM. Ela é diferente da memória privada reservada, que era o número usado antes. Medindo a coluna certa, 20 s depois de abrir:

| Caso | Gerenciador de Tarefas | Reservada |
| --- | --- | --- |
| `glow`, anéis com números fixos, sem rede | 53 MB | 101 MB |
| `glow`, números fixos chegando 1 s depois por outra linha de execução | 53 MB | 102 MB |
| `glow`, leitura real pela rede | 85 MB | 102 MB |
| `wgpu`, números fixos | 116 MB | 384 MB |
| `wgpu`, leitura real | 95 MB | 351 MB |

- O `wgpu` carrega três drivers da AMD (Vulkan, DirectX 12 e OpenGL) e foi descartado.
- No `glow`, a maior parte vem do driver OpenGL da AMD (`atio6axx.dll`).
- Um programa mínimo que só faz a mesma chamada HTTPS com o `ureq` ocupa menos de 1 MB. A rede em si não é o peso.
- Pedindo ao Windows para liberar as páginas paradas (`EmptyWorkingSet`), os dois casos do `glow` caem para 3 MB e se estabilizam em 7 a 8 MB. O app usa de fato cerca de 8 MB. O resto são páginas tocadas uma vez, na abertura do driver e na primeira conexão, que o Windows deixa na RAM até precisar.

**A mudança.** Dois segundos depois de cada leitura, o app chama `K32EmptyWorkingSet` em si mesmo. Medido depois disso: 2,4 MB aos 3 s, 7,9 MB aos 30 s, 8,8 MB aos 60 s, 5,7 MB depois de um hover, com o processador em 0,01%. Os anéis e o cartão continuaram desenhando normalmente.

**O que isso não muda.** A memória reservada continua em cerca de 100 MB, quase toda do driver de vídeo. Ela não ocupa RAM enquanto não é usada.
