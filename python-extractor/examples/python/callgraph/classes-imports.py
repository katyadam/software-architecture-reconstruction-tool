import examples.python.callgraph.classes as classes
from examples.python.callgraph.classes import Divider, sum


class Math:
    def __init__(self, a: int, b: int):
        self.a = a
        self.b = b

    def divide(self):
        divider = Divider(self.a, self.b)
        return divider.divide()

    def sum(self):
        return sum(self.a, self.b)

    def product(self):
        classes.sum(5, 4)
        return product(self.a, self.b)


def product(a: int, b: int) -> int:
    return a * b
