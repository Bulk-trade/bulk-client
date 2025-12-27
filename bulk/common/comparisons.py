import numpy as np

def LT (a: float, b: float, eps: float = 1e-8):
    """a < b with eps error"""
    return (b - a) > eps

def LE (a: float, b: float, eps: float = 1e-8):
    """a <= b with eps error"""
    return (b - a + eps) >= 0.0

def GT (a: float, b: float, eps: float = 1e-8):
    """a > b with eps error"""
    return (a - b) > eps

def GE (a: float, b: float, eps: float = 1e-8):
    """a >= b with eps error"""
    return (a - b + eps) >= 0.0

def isZero (a: float, eps: float = 1e-8):
    """determine if value is equivalent to zero"""
    return np.abs(a) < eps

