"""Parser package for OMGlang.

This package splits the parser functionality into multiple modules to
keep the code organized. The :class:`Parser` class is exposed at the
package level for convenience.


File: __init__.py
Author: Chris Rowles <christopher.rowles@outlook.com>
Copyright: © 2025 Chris Rowles. All rights reserved.
Version: 0.1.1
License: MIT
"""

from .parser import Parser

__all__ = ["Parser"]
