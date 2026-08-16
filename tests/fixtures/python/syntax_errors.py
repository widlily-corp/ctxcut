"""
Python fixture with intentional syntax and indentation errors for parser error recovery testing.
"""

def valid_header_function(x: int, y: int) -> int:
    """This function is well-formed before any syntax breaks."""
    return x + y


def broken_indentation_function(data: list[str]) -> list[str]:
    result = []
for item in data:
      result.append(item.strip())
        return result


def missing_colon_function(a: int, b: int)
    return a * b


@invalid_decorator(
def target_function_in_noisy_file(val: int) -> int:
    """Intact target function surrounded by parsing errors."""
    return val ** 2 + 10


class MalformedClass
    def __init__(self):
        self.value = 42
