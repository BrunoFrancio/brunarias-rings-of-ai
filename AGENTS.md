# Orientação para agentes

- Código e identificadores em inglês; comentários, README e `docs/` em português.
- Cada decisão ou tropeço novo entra em `docs/construcao.md`, com data, o porquê e como foi conferido.
- Commit e PR não levam assinatura de agente, nem `Co-Authored-By` nem link de sessão.
- Antes de concluir mudança de janela, conferir na tela real: o egui e o winit no Windows 11 devolvem a moldura em toda troca de estilo (ver `docs/construcao.md`).
- Testes: `cargo test`. Build: `cargo build --release`.
