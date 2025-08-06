# OMG Language Server and VS Code Extension

This directory contains a minimal Language Server Protocol (LSP) server for
OMG and a matching VS Code extension.

## Setup

1. **Install Python dependencies**

   ```bash
   pip install pygls
   ```

2. **Build the VS Code extension**

   ```bash
   cd vscode-extension
   npm install
   npm run compile
   ```

3. **Link the extension in VS Code**

   From the `vscode-extension` folder run:

   ```bash
   code --install-extension .
   ```

## Features

- Go to definition for top-level `proc` and `alloc` declarations
- Hover information with simple signatures
- Document symbol outline

Only files beginning with the required `;;;omg` header are processed by the
language server.
