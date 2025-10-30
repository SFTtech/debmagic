from dataclasses import dataclass
from typing import Any, Generic, TypeVar

T = TypeVar('T')


@dataclass
class CustomFuncArg(Generic[T]):
    name: str
    type: type[T]
    default: T | None

type CustomFuncArgsT = dict[str, CustomFuncArg[Any]]
