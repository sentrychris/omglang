"""
OMG Language Server entry point.

This server provides basic language features for OMG source files using
`pygls`. It reuses the existing OMG lexer and parser to build a simple
symbol index supporting definition lookup, hover information, and document
symbols.
"""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional
from urllib.parse import urlparse

from pygls.server import LanguageServer
from lsprotocol.types import (
    TEXT_DOCUMENT_DEFINITION,
    TEXT_DOCUMENT_HOVER,
    TEXT_DOCUMENT_DOCUMENT_SYMBOL,
    TEXT_DOCUMENT_DID_OPEN,
    TEXT_DOCUMENT_DID_CHANGE,
    DefinitionParams,
    DidOpenTextDocumentParams,
    DidChangeTextDocumentParams,
    DocumentSymbol,
    DocumentSymbolParams,
    Hover,
    HoverParams,
    Location,
    MarkupContent,
    MarkupKind,
    Position,
    Range,
    SymbolKind,
)

from omglang.lexer import Token, tokenize
from omglang.parser import Parser

print("hi")

@dataclass
class OMGSymbol:
    """Represents a top-level symbol in an OMG file."""

    name: str
    kind: SymbolKind
    uri: str
    line: int
    detail: str


class OMGLanguageServer(LanguageServer):
    """Language server for OMG source files."""

    def __init__(self) -> None:
        super().__init__("omg-ls", "v0.1")
        self.symbols_by_uri: Dict[str, List[OMGSymbol]] = {}
        self.global_symbols: Dict[str, List[OMGSymbol]] = {}
        self.indexed_workspace = False

    def _index_workspace(self) -> None:
        """Parse all `.omg` files under the current workspace."""
        root = self.workspace.root_path
        if not root:
            self.indexed_workspace = True
            return
        for path in Path(root).rglob("*.omg"):
            uri = path.as_uri()
            try:
                text = path.read_text()
            except OSError:
                continue
            self.update_index(uri, text)
        self.indexed_workspace = True

    def update_index(self, uri: str, text: str) -> None:
        """Parse ``text`` and update symbol index for ``uri``.

        Only files that begin with the required ``;;;omg`` header are
        considered valid OMG files.
        """
        if not self._has_header(text):
            return
        symbols = self._parse_symbols(uri, text)
        self.symbols_by_uri[uri] = symbols
        self._rebuild_global_index()

    def _rebuild_global_index(self) -> None:
        self.global_symbols.clear()
        for syms in self.symbols_by_uri.values():
            for sym in syms:
                self.global_symbols.setdefault(sym.name, []).append(sym)

    @staticmethod
    def _has_header(text: str) -> bool:
        """Return ``True`` if the source begins with ``;;;omg``."""
        for line in text.splitlines():
            if line.strip() == "":
                continue
            return line.strip() == ";;;omg"
        return False

    def _parse_symbols(self, uri: str, text: str) -> List[OMGSymbol]:
        """Parse ``text`` into AST and extract top-level symbols."""
        tokens, token_map = tokenize(text)
        eof_line = tokens[-1].line if tokens else 1
        tokens.append(Token("EOF", None, eof_line))
        parser = Parser(tokens, token_map, uri)
        ast = parser.parse()
        symbols: List[OMGSymbol] = []
        for node in ast:
            tag = node[0]
            if tag == "func_def":
                _, name, params, _, line = node
                detail = f"proc {name}({', '.join(params)})"
                symbols.append(
                    OMGSymbol(name, SymbolKind.Function, uri, line - 1, detail)
                )
            elif tag == "decl":
                _, name, _, line = node
                detail = f"alloc {name}"
                symbols.append(
                    OMGSymbol(name, SymbolKind.Variable, uri, line - 1, detail)
                )
            elif tag == "import":
                _, path, alias, _ = node
                self._index_import(self._uri_to_path(uri), path, alias)
        return symbols

    def _index_import(self, base: Path, path: str, alias: str) -> None:
        """Parse an imported file and record its symbols."""
        try:
            # Ensure base is absolute
            base = base.resolve()

            # Strip quotes from import path if it's a string like "./file.omg"
            clean_path = path.strip("\"'")

            # Resolve relative to the importing file's directory
            import_path = (base.parent / clean_path).resolve(strict=False)

            if import_path.suffix != ".omg":
                return

            uri = import_path.as_uri()
            if uri in self.symbols_by_uri:
                return

            # Read the file content
            text = import_path.read_text(encoding="utf-8")
            self.update_index(uri, text)

        except (OSError, ValueError) as e:
            # Log or handle the failure cleanly
            print(f"[LSP] Failed to index import {path} from {base}: {e}")

    @staticmethod
    def _uri_to_path(uri: str) -> Path:
        """Convert a file URI to a :class:`Path` instance."""
        return Path(urlparse(uri).path)


lang_server = OMGLanguageServer()


@lang_server.feature(TEXT_DOCUMENT_DID_OPEN)
def did_open(ls: OMGLanguageServer, params: DidOpenTextDocumentParams) -> None:
    """Index a document when it is opened."""
    ls.update_index(params.text_document.uri, params.text_document.text)


@lang_server.feature(TEXT_DOCUMENT_DID_CHANGE)
def did_change(ls: OMGLanguageServer, params: DidChangeTextDocumentParams) -> None:
    """Re-index a document when it changes."""
    if params.content_changes:
        ls.update_index(params.text_document.uri, params.content_changes[0].text)


@lang_server.feature(TEXT_DOCUMENT_DEFINITION)
def definition(ls: OMGLanguageServer, params: DefinitionParams):
    """Return the definition location for the symbol under the cursor."""
    doc = ls.workspace.get_text_document(params.text_document.uri)
    word = doc.word_at_position(params.position)
    if not word:
        return None
    if not ls.indexed_workspace:
        ls._index_workspace()
    matches = ls.global_symbols.get(word)
    if not matches:
        return None
    sym = matches[0]
    rng = Range(Position(sym.line, 0), Position(sym.line, len(sym.name)))
    return Location(uri=sym.uri, range=rng)


@lang_server.feature(TEXT_DOCUMENT_HOVER)
def hover(ls: OMGLanguageServer, params: HoverParams) -> Optional[Hover]:
    """Return hover information for the symbol under the cursor."""
    doc = ls.workspace.get_text_document(params.text_document.uri)
    word = doc.word_at_position(params.position)
    if not word:
        return None
    if not ls.indexed_workspace:
        ls._index_workspace()
    matches = ls.global_symbols.get(word)
    if not matches:
        return None
    sym = matches[0]
    contents = MarkupContent(kind=MarkupKind.PlainText, value=sym.detail)
    return Hover(contents=contents)


@lang_server.feature(TEXT_DOCUMENT_DOCUMENT_SYMBOL)
def document_symbols(ls: OMGLanguageServer, params: DocumentSymbolParams):
    """Return top-level symbols for the given document."""
    symbols = ls.symbols_by_uri.get(params.text_document.uri, [])
    result: List[DocumentSymbol] = []
    for sym in symbols:
        rng = Range(Position(sym.line, 0), Position(sym.line, len(sym.name)))
        result.append(
            DocumentSymbol(
                name=sym.name,
                kind=sym.kind,
                range=rng,
                selection_range=rng,
                detail=sym.detail,
            )
        )
    return result


def main() -> None:
    """Start the language server."""
    lang_server.start_io()


if __name__ == "__main__":
    main()
