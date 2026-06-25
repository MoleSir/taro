x = 1                    # → lib.__dict__["x"] = 1

def get_x():             # get_x.__globals__ = lib.__dict__
    return x             # → get_x.__globals__["x"] → 1