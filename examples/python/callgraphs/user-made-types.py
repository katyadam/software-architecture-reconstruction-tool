class A():
    def do(self, a: int):
        return a + 5


class B(A):
    pass


def foo(x: A):
    x.do(5)


def foo(x: B):
    x.do(5)


a = A()
foo(a)

typed_a: A = B()
foo(typed_a)

b = B()
foo(b)

typed_b: B = B()
foo(typed_b)
