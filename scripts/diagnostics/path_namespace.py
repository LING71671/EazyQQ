"""Report the physical path visible to this caller without creating or loading files."""
import argparse
import ctypes
import json
import os

parser = argparse.ArgumentParser()
parser.add_argument("path")
args = parser.parse_args()
if os.name != "nt":
    raise SystemExit("This probe requires Windows")
k = ctypes.WinDLL("kernel32", use_last_error=True)
k.CreateFileW.argtypes = [ctypes.c_wchar_p, ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p]
k.CreateFileW.restype = ctypes.c_void_p
k.GetFinalPathNameByHandleW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint, ctypes.c_uint]
k.CloseHandle.argtypes = [ctypes.c_void_p]
handle = k.CreateFileW(os.path.abspath(args.path), 0, 7, None, 3, 0x02000000, None)
if handle == ctypes.c_void_p(-1).value:
    raise OSError(ctypes.get_last_error(), "Cannot open path for provenance")
try:
    buffer = ctypes.create_unicode_buffer(32768)
    length = k.GetFinalPathNameByHandleW(handle, buffer, len(buffer), 0)
    if not length:
        raise OSError(ctypes.get_last_error(), "Cannot resolve physical path")
    physical = buffer.value.removeprefix("\\\\?\\")
    logical = os.path.abspath(args.path)
    print(json.dumps({"logicalPath": logical, "physicalPath": physical,
        "redirected": os.path.normcase(logical) != os.path.normcase(physical),
        "scope": "caller filesystem view; detached process visibility needs separate evidence"}, ensure_ascii=False))
finally:
    k.CloseHandle(handle)
