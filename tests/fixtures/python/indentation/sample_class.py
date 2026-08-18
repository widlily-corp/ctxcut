"""
Python class with nested indentation for AST patching tests.
"""


class IndentedProcessor:
    def __init__(self, initial_value: int = 10) -> None:
        self.value = initial_value

    def process(self, multiplier: int) -> int:
        if multiplier > 0:
            result = self.value * multiplier
        else:
            result = self.value
        return result

    def update_value(self, new_val: int) -> None:
        self.value = new_val
