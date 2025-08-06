"""Parser package for OMGlang.

This package splits the parser functionality into multiple modules to
keep the code organized. The :class:`Parser` class is exposed at the
package level for convenience.
"""

from .parser import Parser

__all__ = ["Parser"]
