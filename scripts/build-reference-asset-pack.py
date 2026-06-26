#!/usr/bin/env python3
"""Compatibility wrapper for the Minecraft asset pack tool."""

from pathlib import Path
import runpy
import sys

tool = Path(__file__).resolve().parent.parent / "tools" / "minecraft_assets" / "asset_pack.py"
sys.argv[0] = str(tool)
runpy.run_path(str(tool), run_name="__main__")
