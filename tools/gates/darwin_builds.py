"""List live Cargo/rustc processes from one Darwin kernel-table snapshot."""
import ctypes
import errno
import os
from pathlib import Path
import struct


def kernel_records():
    libc = ctypes.CDLL(None, use_errno=True)
    mib = (ctypes.c_int * 3)(1, 14, 0)  # CTL_KERN, KERN_PROC, KERN_PROC_ALL
    for _ in range(3):
        length = ctypes.c_size_t()
        if libc.sysctl(mib, 3, None, ctypes.byref(length), None, 0):
            raise OSError(ctypes.get_errno(), "KERN_PROC_ALL size")
        data = ctypes.create_string_buffer(length.value + 65536)
        length.value = len(data)
        if not libc.sysctl(mib, 3, data, ctypes.byref(length), None, 0):
            return data.raw[:length.value]
        if ctypes.get_errno() != errno.ENOMEM:
            break
    raise OSError(ctypes.get_errno(), "KERN_PROC_ALL snapshot")


def builds():
    if (Path.home() / "Library/Logs/FreezeSampler/STALL").exists():
        raise RuntimeError("process lookup stall recorded")
    data = kernel_records()
    # Darwin LP64 <sys/sysctl.h> kinfo_proc and its extern_proc prefix.
    # Fail closed on an ABI change, including a partial or misaligned snapshot.
    stride = 648
    if len(data) % stride:
        raise ValueError("unexpected kinfo_proc size")
    libc = ctypes.CDLL(None)
    # argv[0] can be a symlink such as python3. Compare two kernel records.
    short = ctypes.create_string_buffer(64)  # Darwin struct proc_bsdshortinfo
    pid = os.getpid()
    if (libc.proc_pidinfo(pid, 13, 0, short, len(short)) != len(short)
            or struct.unpack_from("=I", short.raw)[0] != pid):
        raise ValueError("could not read observer identity from PROC_PIDT_SHORTBSDINFO")
    own_name = short.raw[16:32].split(b"\0", 1)[0]
    verified = False
    active = []
    for offset in range(0, len(data), stride):
        state, pid = struct.unpack_from("=b3xi", data, offset + 36)
        name = data[offset + 243:offset + 260].split(b"\0", 1)[0]
        verified |= pid == os.getpid() and name[:16] == own_name
        if state != 5 and name in (b"cargo", b"rustc"):  # SZOMB has exited
            active.append(f"{pid} {name.decode()}")
    if not verified:
        raise ValueError("could not verify Darwin process records against this observer")
    return active


if __name__ == "__main__":
    for process in builds():
        print(process)
