def A(func):
    func()


def B(func):
    A(func)


def C():
    return 5 + 3


def D():
    B(C())
