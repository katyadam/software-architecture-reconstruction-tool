def foo(x):
    y = x
    bar(y)


def bar(a):
    return a + 5


def typed_foo(x: int):
    y = x
    typed_bar(y)


def typed_foo(x: float):
    y = x
    typed_bar(y)


def typed_bar(a: int):
    return a + 5


def typed_bar(a: float):
    return a + 5
