import ctypes
from ctypes import *

raylib = ctypes.CDLL("./libraylib.so.6.0.0")

class Color(Structure):
    _fields_ = [
        ("r", c_ubyte),
        ("g", c_ubyte),
        ("b", c_ubyte),
        ("a", c_ubyte),
    ]


WHITE = Color(255, 255, 255, 255)
BLACK = Color(0, 0, 0, 255)
SKYBLUE = Color(102, 191, 255, 255)
DARKBLUE = Color(0, 82, 172, 255)

raylib.InitWindow.argtypes = [c_int, c_int, c_char_p]
raylib.SetTargetFPS.argtypes = [c_int]
raylib.WindowShouldClose.restype = c_bool
raylib.BeginDrawing.argtypes = []
raylib.EndDrawing.argtypes = []
raylib.ClearBackground.argtypes = [Color]
raylib.DrawText.argtypes = [c_char_p, c_int, c_int, c_int, Color]
raylib.DrawCircle.argtypes = [c_int, c_int, c_float, Color]
raylib.CloseWindow.argtypes = []

screen_width = 800
screen_height = 450

raylib.InitWindow(screen_width, screen_height, b"Python Raylib ctypes Demo")
raylib.SetTargetFPS(60)

x = 100

while not raylib.WindowShouldClose():
    x += 2
    if x > screen_width:
        x = 0
    raylib.BeginDrawing()
    raylib.ClearBackground(WHITE)
    raylib.DrawText(b"Hello Raylib from Python ctypes!", 190, 100, 20, DARKBLUE)
    raylib.DrawCircle(x, 250, 40.0, SKYBLUE)
    raylib.EndDrawing()

raylib.CloseWindow()