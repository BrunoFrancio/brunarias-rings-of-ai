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
