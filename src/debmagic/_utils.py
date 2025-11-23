import io
import re
import shlex
import subprocess
import sys

from typing import TypeVar


class Namespace:
    """
    Store dict as object.
    """

    def __init__(self, **kwargs):
        for name in kwargs:
            setattr(self, name, kwargs[name])

    def __eq__(self, other):
        if not isinstance(other, Namespace):
            return NotImplemented
        return vars(self) == vars(other)

    def __contains__(self, key):
        return key in self.__dict__

    def __hash__(self):
        return hash(self.__dict__)

    def __getattr__(self, key):
        return self.__dict__[key]

    def __getitem__(self, key):
        return self.__dict__[key]

    def get(self, key):
        return self.__dict__.get(key)


def run_cmd(args: list[str] | str, check: bool = True, dry_run: bool = False, **kwargs) -> subprocess.CompletedProcess:
    shell = kwargs.get("shell")
    if shell:
        print(f"debmagic: {args}")
    else:
        if not isinstance(args, (list, tuple)):
            raise ValueError("need list/tuple as command arguments when not using shell=True")
        cmd_pretty = shlex.join(args)
        print(f"debmagic: {cmd_pretty}")

    if dry_run:
        return subprocess.CompletedProcess(args, 0)

    ret = subprocess.run(args, check=False, **kwargs)

    if check and ret.returncode != 0:
        raise RuntimeError(f"failed to execute {cmd_pretty}")

    return ret


def disable_output_buffer():
    # always flush output
    sys.stdout = io.TextIOWrapper(open(sys.stdout.fileno(), "wb", 0), write_through=True)
    sys.stderr = io.TextIOWrapper(open(sys.stderr.fileno(), "wb", 0), write_through=True)


def prefix_idx(prefix: str, seq: list[str]) -> int:
    """
    >>> prefix_idx("a", ["c", "a", "d"])
    1
    >>> prefix_idx("a", ["c", "d", "e", "a"])
    3
    """
    for idx, elem in enumerate(seq):
        if re.match(rf"{prefix}\b", elem):
            return idx

    raise ValueError(f"prefix {prefix!r} not found in sequence")


T = TypeVar("T")


def list_strip_head(data: list[T], head: list[T]) -> list[T]:
    """
    >>> list_strip_head([1,2,3], [])
    [1, 2, 3]
    >>> list_strip_head([1,2,3], [1,2])
    [3]
    >>> list_strip_head([1,2,3], [1,2,3])
    []
    >>> list_strip_head([1,2,3,4,5], [1,2])
    [3, 4, 5]
    """
    idx = 0
    for elem_a, elem_b in zip(data, head):
        if elem_a == elem_b:
            idx += 1
        else:
            break

    return data[idx:]


if __name__ == "__main__":
    import doctest

    doctest.testmod()
