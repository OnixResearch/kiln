def clamp(value, lower, upper):
    if lower > upper:
        raise ValueError("reversed bounds")
    return min(lower, max(value, upper))
