Crosstalk: A General-Purpose CLI for Chat Models
------------------------------------------------

> [!CAUTION]
>
> This project is the successor to `gpt-chat-cli` and is still in early development. There is currently no official release, so expect bugs and undocumented behavior. Additionally, please contact me before contributing. I anticipate releasing an early alpha version around the first of August.

Crosstalk is a command-line utility for interacting with language models (LLMs). It aims to be a general-purpose chat interface for various use cases, including code generation, writing assistance, and question answering.

While there are many command-line clients available, Crosstalk aims to improve upon them in several ways:
- [x] A provider-agnostic architecture that supports multiple LLM providers, both cloud and local
- [x] Streaming responses for real-time interaction
- [x] An interactive REPL with support for command-line editors (e.g., `vim`, `emacs`, etc.)
- [ ] Declarative configuration
- [ ] On-the-fly syntax highlighting
- [ ] Code-block copying
- [ ] Input can be gathered from pipes, heredoc, files, and arbitrary file descriptors (e.g., respects Unix norms and can be used to build ad-hoc command-line tools)
- [ ] Ability to modify model parameters
- [ ] CLI completion

If you have any questions or are interested in contributing, please reach out before getting started.