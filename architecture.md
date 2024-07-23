At a high level, the architecture of Crosstalk is fairly simple:

- **Chat model conversations** are facilitated by chat providers. All chat providers implement the `ChatProvider` trait. This interface boundary abstracts away provider-specific semantics.
- When the program starts, the **enabled** providers are registered in the **registry**. The registry determines which model should serve the user. The user can influence which model serves the request by providing a **model specification** which is **resolved** to a specific provider and model to serve the request. This is accomplished using dynamic dispatch.
- Each provider has specific conditions under which it is **enabled**. Only the enabled providers are registered. The registry is populated during program startup.

All providers are located under `src/providers`. Providers have a two-part structure:
- A "native" API in a module called `api`. This provides a Rust API that mirrors the provider's API semantics and errors as closely as possible while remaining incompatible with the chat provider API.
- A "provider" wrapper located under the `provider` module. This implements the `ChatProvider` interface, performs type conversions, and uses the underlying `api` to resolve the request.

Providers use the `apireq` module, which provides utilities for making API requests and handling low-level errors.