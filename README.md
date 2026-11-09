# Brunarias Rings of AI

Anéis presos na borda direita da tela do Windows mostrando quanto já foi usado da cota do Claude Code, do Codex e do Antigravity. A ideia vem do [Pulse](https://github.com/qunqin24/Pulse), que só existe para macOS. Este é um app próprio, escrito em Rust, e não uma cópia do código dele. Até 09/11/2026 se chamava Anéis de cota.

## O que aparece

- **Em repouso:** um trilho estreito e apagado, com uma cápsula por agente e só a janela de 5 horas de cada um.
- **Mouse perto:** o trilho acende e abre as outras janelas, como a semanal, com um rótulo curto embaixo de cada anel.
- **Mouse em cima:** um cartão com todas as janelas, a barra de uso e quando cada uma renova.
- **Alguma cota em 85% ou mais:** o trilho fica aberto sozinho.

Fora dos anéis, o clique passa para a janela de trás.

## De onde vem cada cota

| Agente | Fonte | Limite |
|---|---|---|
| Claude | login do Claude Code de terminal (`%USERPROFILE%\.claude\.credentials.json`) e o mesmo endereço que o `/usage` usa | o login só se renova rodando o CLI; o app chama `claude update` quando ele vence |
| Codex | o fim dos arquivos de sessão em `%USERPROFILE%\.codex\sessions` | só muda quando o Codex roda neste Windows; janela que renovou sem uso aparece com 0 |
| Antigravity | o servidor local que o próprio Antigravity abre | só responde com o Antigravity aberto; fechado, fica a última leitura até ela vencer |

Nenhum desses endereços é documentado pelos fornecedores e eles podem mudar sem aviso. Os ícones em `assets/` são das marcas de cada agente.

## Rodar

Precisa do Rust (`rustup`, toolchain `stable-msvc`) e das Build Tools do Visual Studio com o kit de C++.

```
cargo run --release
```

Para ler as cotas uma vez no terminal, sem abrir a janela:

```
cargo run --release -- --ler
```

Abre só uma cópia por vez. Testes: `cargo test`.

A história de cada decisão está em [docs/construcao.md](docs/construcao.md).
