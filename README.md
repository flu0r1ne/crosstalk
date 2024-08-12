Crosstalk: A General-Purpose CLI for Chat Models
------------------------------------------------

> [!CAUTION]
>
> This project is the successor to `gpt-chat-cli` and is still in early development. Currently, only an alpha release is available, so expect bugs and undocumented behavior. Please contact me before contributing, as the project is in flux.

Introduction
------------

`xtalk`, pronounced "crosstalk," is a command-line utility for interacting with language models (LLMs). It aims to provide an easy-to-use and highly-configurable chat interface for various use cases, including code generation, writing assistance, and question answering.

Current features include:

- Streaming, real-time output
- An interactive chat REPL with support for command-line editors (e.g., `vim`, `emacs`, etc.)
- Support for both Ollama and OpenAI chat providers
- A composable CLI interface:
    + Input can be gathered from pipes, heredoc, and arbitrary file descriptors
    + Listings can produce JSON- and awk-compatible output
- Declarative configuration

Installation
------------

The `xtalk` alpha release only supports Linux.

## Installing with Cargo

```
cargo install xtalk@0.0.1-alpha.2
```

### Building From Source

First, clone the repository and change into the root of the project:
```
git clone https://github.com/flu0r1ne/crosstalk
cd crosstalk
```

Next, compile the binary:
```
cargo build --release
```

Finally, copy the binary into the `/usr/local/bin` directory:

```
cp target/release/xtalk /usr/local/bin/xtalk
```

User Guide
----------

### Getting Started

Before starting, you must activate one of the chat providers. Crosstalk is not distributed with its own LLMs, so it needs access to them through one of the providers' APIs. Currently, there are two options: OpenAI and Ollama. Using [OpenAI](https://www.openai.com/), you have access to flagship models, including `gpt-4o` and `gpt-4o-mini`. [Ollama](https://ollama.com/) allows you to run LLMs locally, preserving data privacy. Under certain conditions, these providers may automatically activate.

You can check the activation status using the `xtalk list providers` command:

```bash
$ xtalk list providers
PROVIDER  PRIORITY  ACTIVATED
ollama    15        yes      
openai    10        yes      
```

In the listing above, we see both Ollama and OpenAI are active. If none of the providers are active, please visit their respective sections.

### Basic Usage

Once you have enabled a provider, the `list` subcommand can list all accessible models:

```
$ xtalk list models
MODEL                     PROVIDER  CONTEXT
llama2:7b                 ollama    unknown
codellama:7b              ollama    unknown
mixtral:8x7b              ollama    unknown
llama2-uncensored:latest  ollama    unknown
codegemma:7b              ollama    unknown
gemma:2b                  ollama    unknown
gemma:7b                  ollama    unknown
llama3:latest             ollama    unknown
gpt-4o-mini               openai    128000 
gpt-4o                    openai    128000 
gpt-4-turbo               openai    128000 
gpt-4                     openai    8192   
gpt-3.5-turbo             openai    16385  
```

The model column provides a list of models with which we can start a chat.

> Note: For a model to be available through Ollama, you must first download it through `ollama pull <model>`.

To select a model off that list and start a chat, use `xtalk chat -m <MODEL>`. This will drop the user into an interactive shell:

```
$ xtalk chat -m gemma:2b
[#] Hello
[gemma:2b] Hello! How can I assist you today?

Is there anything I can help you with?
```

This supports standard Emacs-style bindings. To exit the shell, press `Control + D` at any point.

For a single completion, an initial message can be specified as the first positional argument:

```
$ xtalk chat -m gemma:7b "In one sentence, who is Joseph Weizenbaum?"
Joseph Weizenbaum was a pioneering computer scientist and psychologist who developed the first conversational AI, ELIZA.
```

Alternatively, you can specify the initial message and drop into an interactive shell with `-i`:

```
$ xtalk chat -m gemma:7b -i "In one sentence, who is Joseph Weizenbaum?"
Joseph Weizenbaum was a pioneering computer scientist and psychologist who developed the first conversational AI, ELIZA.

[#] 
```

### Default Model

If you start a chat without specifying a model, `xtalk` will start a chat with the *default model*. The default model will be the preferred provider's default model. If you have enabled the OpenAI provider, `gpt-4o-mini` will be the default model. If only Ollama is enabled, you must manually specify a default model. See the default model section for more details.

If a command is unspecified, Crosstalk will start a chat with the default model:

```
$ xtalk
[#] Hi!
[gpt-4o-mini] Hello! How can I assist you today?
```

Documentation
-------------

### Interactive Chat

The interactive chat has support for both slash commands and keybindings. Keybindings can manipulate text while slash commands manipulate the session.

**Slash Commands:**

If the prompt begins with a `/`, it is interpreted as a slash command. These commands change aspects of the chat rather than being interpreted by the model. There are currently three slash commands:

| Command | Function                                                                                                                           |
|---------|------------------------------------------------------------------------------------------------------------------------------------|
| /clear  | Clears the chat buffer. The model will interpret the next message as the first message in the conversation.                        |
| /edit   | Launches an interactive editor. After the editor quits, any content written to the file will become the content of the next message. |
| /exit   | Exits the shell                                                                                                                    |

**Keybindings:**

Crosstalk currently uses Emacs-style keybindings for text manipulation. Although this is not an exhaustive list of available keybindings, these are likely to be preserved between releases:

| Keybinding | Function                          |
|------------|-----------------------------------|
| C-j        | Insert a newline                  |
| C-e        | Open the editor                   |
| C-d        | Exit the shell (EOF)              |
| Home       | Move to the beginning of the line |
| End        | Move to the end of the line       |
| C-l        | Clear the screen                  |
| Tab        | Perform tab completion            |
| C-k        | Remove text from the cursor to the end of the line   |
| C-u        | Remove text from the cursor to the start of the line |

**Launching a Text Editor:**

An external text editor can be launched with `C-e` or the `/edit` command as detailed above. This external editor is invoked on a temporary file when `C-e` or `/edit` is specified. The editor should exit normally and write the content of the next prompt to a file. This content is then used in the conversation.

The editor can be specified using one of the following mechanisms. The first one found is used:

- The `editor` field in the configuration file
- The `EDITOR` environment variable
- The `editor` Debian command
- `vim`, `emacs`, `vi`, or `nano`, whichever is found first

### Model Specification

Models are specified using a *model spec*, which consists of the model name, optionally preceded by a provider. For example, an unambiguous model specification is `ollama/gemma:2b`, which means access the `gemma:2b` model through the `ollama` provider. The *model spec* can also just consist of the model name `gemma:2b`, in which it is considered ambiguous. In this case, a provider for `gemma:2b` will automatically be selected. If multiple providers exist, the user's preferred provider will be used. See the Provider Preference section for more details. If the *model spec* is unspecified in the `chat` command, the default model is used.

| Model Spec       | Meaning                                                         |
|------------------|----------------------------------------------------------------|
| ollama/llama3:7b | Access llama3:7b through the Ollama provider                   |
| llama3:7b        | Access llama3:7b through the user's preferred provider        |
| None             | Use the default model                                           |

Example:

```
$ xtalk chat -m ollama/llama3:7b "Hi Llama3!"
```

> Note: Currently, there are no instances where two providers serve the same model. However, this may change in the future.

### Providers

Providers are entities that provide chat services to Crosstalk. Providers have their own distinct APIs, which are integrated into the common Crosstalk interface.

Crosstalk currently supports two providers:
- OpenAI
- Ollama

Each provider has a Provider ID. This mnemonic is used to refer to them through the API. For OpenAI, this is `openai`, and for Ollama, this is `ollama`.

#### Activation

Providers require user-specified parameters to function, such as an API key. By default, providers will automatically activate if their activation criteria are met. This behavior can be disabled by deactivating providers. Alternatively, a provider can be forcibly enabled. If the activation criteria are unmet, Crosstalk will throw an error.

| Provider | Parameters                        | Automatic Activation Criteria                              |
|----------|-----------------------------------|------------------------------------------------------------|
| ollama   | Ollama API Base URL (defaults to localhost:11434) | Responds to a request during startup*                      |
| openai   | OpenAI API Key                    | The `OPENAI_API_KEY` environment variable is defined       |

\* This can be disabled by forcibly enabling the provider.

##### Activating OpenAI

To activate the OpenAI provider, you must provide `xtalk` with an OpenAI API key:

1. Visit [https://platform.openai.com/api-keys](https://platform.openai.com/api-keys).
2. Create a new secret key:
   - Select "Restrict key permissions."
   - Choose a name for the key, for example, "xtalk."
   - Enable write permissions for "Model capabilities" and read permissions for "Models."
     - **Note:** Future features may require additional permissions. You can either grant all permissions (not recommended) or generate a new key when additional permissions are necessary.
3. Create a configuration file under `~/.config/xtalk/config`:
   ```bash
   mkdir -p ~/.config/xtalk
   touch ~/.config/xtalk/config.toml
   ```
4. Populate the `api_key` field under the `providers.openai` section:
   ```toml
   [providers.openai]
   api_key = "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
   ```

##### Activating Ollama

The Ollama provider will automatically activate if the Ollama server is running on `localhost:11434`. If the API endpoint differs from this default, you can change it in the configuration file as follows:

```toml
[providers.ollama]
api_base = "http://my-server.com:42"
```

#### Provider Preference

When a model is provided by two providers, the provider that serves the request will be the preferred provider. This precedence is established through the provider priority, with providers with higher priority being selected to serve the request. The priority is an unsigned 8-bit number, with the lowest priority being 0 and the highest being 255. If two providers have equal priority, the selected provider is implementation-dependent.

| Provider | Default Priority |
|----------|------------------|
| ollama   | 15               |
| openai   | 10               |

> Note: All local providers will have a default priority of 15, and all remote providers will have a default priority of 10. This ensures local providers are preferred by default.

### Model Defaults

The default model is the model which is used if the user does not specify a preference when a chat is envoked. The user can specify which model is selected by default in the configuration file. If a default is not specifed, the provider with the highest preference sets the default model. There are two ways to specify the default model, explicity or via the preferred provider.

#### Setting the default model explicity

The default model can be set using the `default_model` field in the configuration file.

Example 1:
```toml
default_model = "gpt-4o-mini"
```

Example 2:
```toml
default_model = "ollama/llama3:7b"
```

#### Provider default models

If the default model is not explicity set, the highest preference provider sets the default model. If the highest preference provider does not set a default model, the default model for the provider with the next highest priority is used. It may be preferred to set the default model per-provider rather than setting the default model globally since if a provider becomes unavailable, the default model will automatically fallback to a better default.

Example 1:
```toml
[providers.ollama]
priority = 20
default_model = 'llama:7b'

[providers.openai]
priority = 10
default_model = 'gpt-4o'
```

In the above example, `ollama/llama:7b` would be the default model unless ollama became unavailable. (E.g. the client was not running, meaning that the activation criteria are not met.) In this case, `openai/gpt-4o` would be the default model.

### Composability

Crosstalk respects pipes and redirects, so you can use it in combination with other command-line tools:

Example 1:
```
$ printf "What is smmsp in /etc/group?\n$(cat /etc/group | head)" | xtalk chat -m gpt-4o-mini

In the context of the `/etc/group` file in a Unix-like operating system, `smmsp` typically refers to a system user group related to the Simple Mail Transfer Protocol (SMTP) message submission program, often specifically associated with mail transfer agents (MTAs) like Sendmail.

Here's a breakdown of the entry:
...
```

Example 2:
```
$ xtalk chat -m gpt-4o-mini "Write rust code to find the average of a list" > average.rs
$ cat average.rs

Here's an example Rust code to find the average of a list of numbers:

fn main() {
    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter().sum();
    let count = numbers.len();
    let average = sum / count as i32;
    println!("The average is {}", average);
}

This code creates a vector of numbers, calculates the sum of the numbers using the `iter()` method and the `sum()` method, counts the number of elements in the vector using the `len()` method, and then calculates the average by dividing the sum by the count. Finally, it prints the average to the console.
```

If `xtalk` detects the `stdin` or `stdout` are redirected, it will operate in one-shot mode. The prompt is the first message in the conversation and the model will preform a single completion before exiting.

## Configuration

Configuration information is stored in a TOML file. The following paths are searched for the configuration file. The first available file is used:

- `~/.config/xtalk/config.toml`
- `~/.xtalk.toml`
- `/etc/xtalk.toml`

If any option is left unspecified in the configuration, a reasonable default is chosen.

### Example configuration:
```toml
# Specifies the command used to launch an external editor.
# This should specify a binary to be used as the external editor. It can either be
# an absolute path to a binary or a command in the PATH environment variable.
# It should accept a file as the first argument. If the editor exits with a zero status,
# the content in the file will be used for a prompt.
editor = "vim"

# Specifies the default model.
# It should be set in the form of a model spec, such as "gpt-4o-mini".
default_model = "gpt-4o-mini"

# Specifies the keybindings to be used within the chat REPL.
# Acceptable values are "vi" or "emacs". By default, Emacs-style bindings are used.
keybindings = "emacs"

# Configuration for the providers.
[providers]
[providers.ollama]
# The activation policy for Ollama.
# Acceptable values are "auto", "enabled", or "disabled".
activate = "auto"

# Specifies the default model to be used when Ollama is the preferred provider.
default_model = "llama2:7b"

# Specifies the base URL for the Ollama API.
api_base = "http://localhost:11434"

# Sets the priority for the Ollama provider.
priority = 15

[providers.openai]
# The activation policy for OpenAI.
# Acceptable values are "auto", "enabled", or "disabled".
activate = "auto"

# Specifies the default model to be used when OpenAI is the preferred provider.
default_model = "gpt-4"

# Sets the OpenAI API key.
# This takes precedence over the OPENAI_API_KEY environment variable, if set.
api_key = "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"

# Sets the priority for the OpenAI provider.
priority = 10
```

### Main Configuration Options

The main configuration structure includes settings for the external editor, default model, keybindings, and provider configurations.

#### Editor
- **Description**: Specifies the command used to launch an external editor.
- **Type**: `String`
- **Default**: See the external editor section above.
- **Example**:
  ```toml
  editor = "vim"
  ```

#### Default Model
- **Description**: Specifies the default chat model and overrides defaults specified by other providers.
- **Type**: `String`, must be a model spec
- **Example**:
  ```toml
  default_model = "gpt-4o-mini"
  ```

#### Keybindings
- **Description**: Specifies the keybindings to be used within the chat REPL.
- **Type**: `String` (can be "emacs" or "vi")
- **Default**: `emacs`
- **Example**:
  ```toml
  keybindings = "emacs"
  ```

### Provider Configuration

Provider settings are nested under the `[providers]` section. Each provider, such as Ollama and OpenAI, has its own configuration settings.

#### Ollama Provider
- **Section**: `[providers.ollama]`
- **Fields**:
  - `activate`
    - **Description**: The activation policy for Ollama.
    - **Type**: `String` (can be "auto", "enabled", or "disabled")
    - **Default**: `auto`
  - `default_model`
    - **Description**: Specifies the default model to be used when Ollama is the preferred provider.
    - **Type**: `String`
  - `api_base`
    - **Description**: Specifies the base URL for the Ollama API.
    - **Type**: `String`
  - `priority`
    - **Description**: Sets the priority for the Ollama provider.
    - **Type**: `Integer`
    - **Default**: `15`
- **Example**:
  ```toml
  [providers.ollama]
    activate = "auto"
    default_model = "llama:7b"
    api_base = "http://localhost:11434"
    priority = 15
  ```

#### OpenAI Provider
- **Section**: `[providers.openai]`
- **Fields**:
  - `activate`
    - **Description**: The activation policy for OpenAI.
    - **Type**: `String` (can be "auto", "enabled", or "disabled")
    - **Default**: `auto`
  - `default_model`
    - **Description**: Specifies the default model to be used when OpenAI is the preferred provider.
    - **Type**: `String`
  - `api_key`
    - **Description**: Sets the OpenAI API key. This takes precedence over the OPENAI_API_KEY environment variable, if set.
    - **Type**: `String`
  - `priority`
    - **Description**: Sets the priority for the OpenAI provider.
    - **Type**: `Integer`
    - **Default**: `10`
- **Example**:
  ```toml
  [providers.openai]
    activate = "auto"
    default_model = "gpt-4"
    api_key = "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
    priority = 10
  ```

Roadmap
-------

I believe there is a lot of improvement to be made in in-place file editing. E.g. the ability to use LLMs for code generation and editing. Although to get to this stage, the groundwork for a chat client must be laid. This is the focus for release `0.0.1`.

This is a rough plan for the first release. Items on this list may be reordered or reinvisioned:

- `0.0.1-alpha.3`
    + Add `/model` command to the REPL which allows the user to change
    the active model
    + Allow REPLs to be launched without a default model
- `0.0.1-alpha.4`
    + Use Control-C to cancel model output
- `0.0.1-alpha.5`
    + Add syntax highlighting
- `0.0.1-alpha.6`
    + Automatically check for updates, alert users when critical updates occur
- `0.0.1-alpha.7`
    + Add the ability to save and load chat dialogs
- `0.0.1-beta.1`
    + Cross-platform testing

Support and Stability
---------------------

Crosstalk only has issued an alpha release. Therefore, all components should be considered unstable. When version 1 is released, the CLI and configuration will be stabilized.

Crosstalk relies on model provider's APIs. Crosstalk can only be as stable as the upstream providers. Some providers do not support the ability to list available models. This means Crosstalk will have to be updated contiguously in order to access new models.

| Platform   | Alpha | Beta    | Release Target       | Support |
|------------|-------|---------|----------------------|---------|
| GNU/Linux  | Yes   | Planned | Planned (0.0.1-rc.1) | Yes     |
| OpenBSD    | No    | Planned | Planned (0.0.1-rc.1) | Yes     |
| FreeBSD    | No    | No      | As-is                | Yes     |
| MacOS      | No    | Planned | Planned (0.0.1-rc.1) | Yes     |
| Windows 10 | No    | No*     | No*                  | No*     |
| WSL 2      | No**  | No**    | Planned (0.0.1-rc.1) | Yes     |

\* Due to limited bandwidth, Windows 10 is currently deprioritized. This may change in the future.

\*\* WSL 2 is assumed to be fully compatible with GNU/Linux so there is no offical pre-release testing beyond that done on GNU/Linux.
