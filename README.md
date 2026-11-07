# Anéis de cota

Anéis presos na borda da tela do Windows mostrando quanto sobra da cota do Claude Code e do Codex. A ideia vem do [Pulse](https://github.com/qunqin24/Pulse), que só existe para macOS. Este é um app próprio, escrito em Rust, e não uma cópia do código dele.

Estado em 06/11/2026: só a prova da janela, com números fixos. Ainda não lê cota nenhuma.

## Rodar

Precisa do Rust (`rustup`, toolchain `stable-msvc`) e das Build Tools do Visual Studio com o kit de C++.

```
cargo run --release
```

A janela aparece na borda direita do monitor principal. Com o mouse sobre os anéis, aparece o cartão. Fora deles, o clique passa para a janela de trás.

A história de cada decisão está em [docs/construcao.md](docs/construcao.md).
