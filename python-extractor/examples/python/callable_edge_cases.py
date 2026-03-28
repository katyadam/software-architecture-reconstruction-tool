from typing import List


# Edge case 1: Lambda assigned to a variable — NOT extracted as a callable;
# lambdas are not function_definition nodes so the extractor ignores them
greet = lambda name: f"Hello, {name}"


# Edge case 2: Nested function def — only the outer function is extracted;
# the inner def is inside the outer function body and NOT captured at module level
def make_adder(x: int):
    def add(y: int) -> int:
        return x + y
    return add


# Edge case 3: Class with a @staticmethod — the method IS extracted as a
# callable, but the @staticmethod decorator is not reflected in any field
# (is_constructor stays False, is_async stays False, no decorator flag)
class MathUtils:

    @staticmethod
    def square(n: int) -> int:
        return n * n

    def double(self, n: int) -> int:
        return n * 2
