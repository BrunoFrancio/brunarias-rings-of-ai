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
- `aneis-de-cota --claude` lê uma vez e imprime no terminal, sem abrir a janela. Em 09/11 virou `--ler`, que lê as duas fontes.

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

## 09/11/2026: a cota do Codex

**De onde vem.** O Codex grava em cada sessão (`%USERPROFILE%\.codex\sessions\AAAA\MM\DD\rollout-*.jsonl`) um evento `token_count` com `rate_limits`. Cada janela traz `used_percent`, `window_minutes` (300 para 5 horas, 10080 para 7 dias) e `resets_at` em segundos desde 1970. Não precisa de rede nem de login. É a mesma fonte do widget de limites do Brunarias Dock. Só a pasta do Windows é lida, porque o Codex do WSL parou de ser usado em outubro.

**O que foi feito.**

- `src/quota.rs` guarda a janela de cota, comum ao Claude e ao Codex.
- `src/codex.rs` pega o último `rate_limits` não nulo entre as linhas de uma sessão, ignorando linha quebrada, e transforma cada janela em anel. Lê as 8 sessões mais recentes até achar uma com cota.
- A linha de execução lê as duas fontes a cada 3 minutos. O erro de uma não esconde a outra: os anéis da que leu aparecem, e o erro da outra vai para o cartão.
- **Leitura vencida.** O número do Codex só muda quando o Codex roda. Se a hora de renovação já passou, o uso de agora é desconhecido. O anel fica sem arco, com um `?`, e o cartão diz "abra o Codex para atualizar". Nunca aparece um 0% presumido. A regra veio do widget do Dock e vale também para o Claude.
- A janela passou para 340 x 320. O cartão (268 px), a folga e o trilho somam 332 px, e com 320 o começo do texto saía cortado. Isso vinha de antes e só ficou visível com o cartão mais cheio. O trilho escuro acompanha o número de anéis.
- `--claude` virou `--ler`, que imprime as duas fontes e marca a leitura vencida.

**Testes.** Sete novos, escritos antes da implementação. Contra funções vazias, quatro falharam pelo motivo esperado (RED). Os outros três passaram porque a função vazia já devolve "nada" ou "não vencida", que era o esperado para eles. Depois da implementação, os 13 do projeto passaram (GREEN). Os valores de data esperados foram calculados à parte, em Python, e não pelo próprio código.

**Conferido no PC de casa.**

- `--ler`: Claude 20% em 5 h e 31% em 7 dias; Codex 9% em 7 dias e a janela de 5 h marcada como vencida (o último uso do Codex foi em 07/11).
- Captura da borda da tela: quatro anéis, o terceiro com `?`, e o cartão inteiro no hover.

**Memória, medida de novo.** A primeira medida deu 21 MB contra 10 MB da versão anterior, e parecia culpa do Codex. Rodando as duas versões lado a lado por 4 minutos, gastaram o mesmo: sobem cerca de 1 MB a cada 30 s, chegam a 12 a 17 MB antes da leitura seguinte e voltam a 4,5 MB quando o app libera as páginas paradas. A diferença era ruído entre rodadas. A medida de 07/11 ("se firma em 7 a 8 MB") olhou só o primeiro minuto. O ciclo real vai de 4,5 MB a cerca de 15 MB.

**Limite conhecido.** Uso do Codex fora deste Windows não aparece até o Codex rodar aqui de novo.

## 09/11/2026: o visual

**Como foi decidido.** Bruno pediu para desenhar antes de codar. A proposta foi feita como maquete em HTML, em três estados (repouso, mouse em cima e alerta), e aprovada depois de uma troca: ele pediu os ícones originais no lugar dos que eu tinha desenhado.

**O que mudou.**

- **Identificação.** O trilho tem um bloco por agente, com o ícone dele no topo, uma linha separando os dois blocos e o percentual dentro de cada anel. Embaixo de cada anel aparece "5h" ou "7d".
- **Ícones.** São os dos apps instalados neste PC (o tile de 48 px do pacote do Claude e o do Codex), copiados para `assets/` e embutidos no executável com `include_bytes!`. Pertencem à Anthropic e à OpenAI, e o repositório é só local. Para ler o PNG entrou a crate `image`, só com o formato PNG.
- **Discreto.** Em repouso, o trilho fica com 45% de opacidade e fundo quase transparente. Ele acende em 0,25 s (`animate_value_with_time`) quando o mouse chega a menos de 120 px, ou quando alguma janela que não venceu passa de 85%.
- **Cartão.** Tem um título por agente, uma barra por janela com a cor do uso, o percentual à direita e, embaixo, "renova em 2 h 10" quando falta menos de um dia, ou o dia da semana e a hora quando falta mais. O erro de um agente aparece com quebra de linha dentro do bloco dele.
- **Fonte.** Segoe UI, a do Windows, lida de `C:\Windows\Fonts`, com a Semibold nos títulos. Se faltar, o app cai na fonte do egui.

**Testes.** O texto de renovação é regra nova e ganhou três testes antes do código: menos de 1 hora, menos de 1 dia (com e sem minutos) e 1 dia ou mais. Os três falharam contra a função vazia (RED) e passaram depois (GREEN), e o projeto todo ficou com 16 testes. O dia da semana de 11/11/2026 foi conferido à parte, em Python.

**Conferido no PC de casa.** Foram capturados três estados da borda da tela: repouso apagado, aceso com o mouse perto e cartão aberto com o mouse no trilho. A memória ficou em 6,2 MB aos 10 s, 8,1 MB aos 30 s e 12 MB aos 60 s, na mesma faixa de antes. O executável passou a ter 8,1 MB. O estado de alerta não foi visto na tela, porque nenhuma cota passava de 85%.

## 09/11/2026: a moldura que piscava

**O que Bruno viu.** Numa gravação da tela, num quadro, a janela apareceu com a barra de título do Windows, os botões de minimizar e fechar e uma borda branca, bem quando o mouse entrava ou saía do trilho.

**A causa.** Para deixar o clique passar, o app usava o comando `MousePassthrough` do egui. No winit, esse comando liga `WS_EX_TRANSPARENT | WS_EX_LAYERED` e reaplica todos os estilos da janela, o que devolve a moldura. O app a tirava de novo no quadro seguinte, mas um quadro já aparecia.

**A correção.**

- O app liga e desliga os dois bits direto, com `SetWindowLongPtrW(GWL_EXSTYLE)`, sem passar pelo winit. O estado interno só muda quando a janela foi encontrada, para não ficar marcado como transparente sem estar.
- A limpeza da moldura passou a rodar a cada quadro e só age quando a moldura volta. Sem a reaplicação a cada troca, ficou visível que o winit devolve os estilos de moldura nos primeiros quadros, depois da primeira limpeza, embora nada apareça na tela.

**Como foi conferido.** O mesmo roteiro rodou nas duas versões: gravação da borda da tela a 60 quadros por segundo com o `ffmpeg` (`gdigrab`), enquanto o cursor entrava e saía do trilho 8 vezes. A medida foi o brilho médio da faixa de 32 px do topo da janela em cada quadro. A barra de título branca passa de 100, e o fundo escuro fica entre 40 e 52.

| Versão | Quadros | Quadros com a barra | Brilho máximo |
| --- | --- | --- | --- |
| antes (`4d319a2`) | 455 | 2 | 164 |
| depois | 454 | 0 | 50 |

Depois da correção, o estilo da janela ficou `96030000` (popup, sem barra de título), e os bits estendidos foram lidos a cada troca: `C0038` fora do trilho (clique passa) e `40018` sobre ele (clique pega). Os 16 testes seguem passando. Não houve teste automático novo: o defeito é da janela do Windows, e a prova é a gravação.

## 09/11/2026: trilho menor e o Antigravity

**O pedido.** Bruno pediu o trilho menor, com só a janela de 5 horas quando ele está apagado, e o Antigravity como terceiro agente. Antes de codar, o desenho foi aprovado numa maquete.

**De onde vem a cota do Antigravity.** Ela não fica em arquivo. Quem responde é o `language_server.exe` que o Antigravity abre, e só enquanto ele está aberto. O caminho é o mesmo do CodexBar (`docs/antigravity.md` do repositório dele):

- achar o processo `language_server*` cuja linha de comando tem `--app_data_dir antigravity`;
- tirar dela o `--csrf_token`;
- achar as portas em escuta do processo;
- chamar `POST https://127.0.0.1:<porta>/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary` com o corpo `{}` e os cabeçalhos `X-Codeium-Csrf-Token` e `Connect-Protocol-Version: 1`.

A resposta vem em grupos (Gemini, Claude e GPT, e Gemini 4 Argon), e cada grupo traz janelas de 5 horas e semanais com `remainingFraction` e `resetTime`. A outra rota, pelo login do Google, foi descartada: um tópico no fórum do Google diz que ela pode levar à suspensão da conta.

**Como ficou no código** (`src/antigravity.rs`):

- A linha de comando de outro processo vem de `NtQueryInformationProcess` com a classe 60 (`ProcessCommandLineInformation`), sem PowerShell e sem ler a memória do outro processo.
- Os processos vêm do `CreateToolhelp32Snapshot`, e as portas, do `GetExtendedTcpTable`.
- O servidor usa um certificado próprio em 127.0.0.1, então a verificação de TLS fica desligada só nessa chamada.
- Na versão 2.21.1, a porta muda e às vezes some por alguns segundos logo depois de abrir. O app tenta todas as portas em escuta do processo.
- Com o Antigravity fechado, o app mantém a última leitura boa. A regra da leitura vencida cuida dela quando a janela renova. Se o app abrir com o Antigravity fechado, aparece "abra o Antigravity para ler a cota".
- No protobuf, um valor zero some do JSON. Por isso, `remainingFraction` ausente conta como cota esgotada.

**Trilho menor.** O trilho passou de 52 para 38 px e os anéis, de 38 para 28. Em repouso aparece só a janela de 5 horas de cada agente, sem rótulos. Com o mouse perto, ou com alguma cota acima de 85%, aparecem todas as janelas. No Antigravity, o rótulo leva a inicial do grupo: G5h, G7d, C5h, C7d e A7d. O cartão ficou com 290 px, para caber "Gemini 4 Argon 7 dias".

**Um defeito achado no caminho.** Logo depois de abrir, a posição da janela que o egui informa ainda é a padrão, no canto superior esquerdo. Com o cursor parado ali, o trilho abria sozinho. A posição agora vem do `GetWindowRect`.

**Testes.** Cinco novos em `src/antigravity.rs`, escritos antes. Contra funções vazias, três falharam (RED). Os dois de recusa passaram porque a função vazia já recusa tudo. Depois da implementação, os 21 do projeto passaram (GREEN).

**Conferido no PC de casa.**

- `--ler` com o Antigravity aberto: cinco janelas, com Gemini e Claude e GPT em 0% e Argon em 9%, em 0,6 s.
- `--ler` com o Antigravity fechado: a mensagem para abrir.
- Captura dos três estados: repouso com só a janela de 5 h, mouse perto com tudo aberto, e cartão.
- Memória: 7,3 MB aos 10 s, 8,4 MB aos 30 s e 9,6 MB aos 60 s.
- **Não conferido na tela:** o app mantendo a última leitura depois de o Antigravity fechar, porque o ciclo de leitura é de 3 minutos.
