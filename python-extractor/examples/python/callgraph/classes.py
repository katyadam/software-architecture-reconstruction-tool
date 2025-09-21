class Divider:
    def __init__(self, a: int, b: int):
        self.a = a
        self.b = b

    def divide(self) -> float:
        if self.dividable():
            return self.a / self.b
        else:
            sum(a=self.a, b=self.b)

    def dividable(self) -> bool:
        return self.b != 0


def sum(a: int, b: int) -> int:
    return a + b
