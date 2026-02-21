# OpenStrap
A fork of the "pekora.rip" bootstrapper, edited to be suitable for *any* revivals.

Currently all domain leads to "pekora.zip", you can change them to any domain. 
As long as the revivals you are trying to port met these criterias:

- all archived clients should be in the .zip format
- The client filename should be named "(version)-(CLIENTFILENAMEPREFIX)(year).zip"
- the enpoint for the client "version" (e.g. https://setup.yourrev.xyz/version) is returning the correct version.

## Compiling
If you'd like, you can compile the bootstrapper yourself.

1. Get the Rust toolchain (https://rustup.rs/).
2. On the root of the source, execute `cargo build --release`. The binary will now be located in `target/release/`.