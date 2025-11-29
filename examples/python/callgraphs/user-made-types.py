class A():
    def do(self, a: int):
        return a + 5


class B(A):
    def do(self, a: int):
        return a + 7


def foo(x: A):
    x.do(5)


def foo(x: B):
    x.do(5)
