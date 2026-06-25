class Obj:
    def __new__(cls):
        return 1

    def __init__(self):
        print("init")

x = Obj()
print(x)