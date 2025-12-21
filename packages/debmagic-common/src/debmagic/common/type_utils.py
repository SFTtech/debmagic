from typing import IO, Iterable, Text

IterableDataSource = bytes | Text | IO[Text] | Iterable[Text] | Iterable[bytes]
