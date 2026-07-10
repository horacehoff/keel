# The Computer Language Benchmarks Game
# http://benchmarksgame.alioth.debian.org/
#
# originally by Kevin Carson
# modified by Tupteq, Fredrik Johansson, and Daniel Nanz
# modified by Maciej Fijalkowski
# 2to3

import sys


def combinations(l):
    result = []
    for x in range(len(l) - 1):
        ls = l[x + 1 :]
        for y in ls:
            result.append([l[x], y])
    return result


PI = 3.14159265358979323
SOLAR_MASS = 4 * PI * PI
DAYS_PER_YEAR = 365.24

sun = {"x": 0.0, "y": 0.0, "z": 0.0, "vx": 0.0, "vy": 0.0, "vz": 0.0, "mass": SOLAR_MASS}
jupiter = {
    "x": 4.84143144246472090,
    "y": -1.16032004402742839,
    "z": -0.103622044471123109,
    "vx": 0.00166007664274403694 * DAYS_PER_YEAR,
    "vy": 0.00769901118419740425 * DAYS_PER_YEAR,
    "vz": -0.0000690460016974260023 * DAYS_PER_YEAR,
    "mass": 0.000954791938424326609 * SOLAR_MASS,
}
saturn = {
    "x": 8.34336671824457987,
    "y": 4.12479856412430479,
    "z": -0.403523417114321381,
    "vx": -0.00276742510726862411 * DAYS_PER_YEAR,
    "vy": 0.00499852801234917238 * DAYS_PER_YEAR,
    "vz": 0.0000230417297573763929 * DAYS_PER_YEAR,
    "mass": 0.000285885980666130812 * SOLAR_MASS,
}
uranus = {
    "x": 12.8943695621391310,
    "y": -15.1111514016986312,
    "z": -0.223307578892655734,
    "vx": 0.00296460137564761618 * DAYS_PER_YEAR,
    "vy": 0.00237847173959480950 * DAYS_PER_YEAR,
    "vz": -0.0000296589568540237556 * DAYS_PER_YEAR,
    "mass": 0.0000436624404335156298 * SOLAR_MASS,
}
neptune = {
    "x": 15.3796971148509165,
    "y": -25.9193146099879641,
    "z": 0.179258772950371181,
    "vx": 0.00268067772490389322 * DAYS_PER_YEAR,
    "vy": 0.00162824170038242295 * DAYS_PER_YEAR,
    "vz": -0.0000951592254519715870 * DAYS_PER_YEAR,
    "mass": 0.0000515138902046611451 * SOLAR_MASS,
}

bodies = [sun, jupiter, saturn, uranus, neptune]
pairs = combinations(bodies)


def advance(dt, n, bodies=bodies, pairs=pairs):
    for _ in range(n):
        for b1, b2 in pairs:
            dx = b1["x"] - b2["x"]
            dy = b1["y"] - b2["y"]
            dz = b1["z"] - b2["z"]
            mag = dt * ((dx * dx + dy * dy + dz * dz) ** -1.5)
            b1m = b1["mass"] * mag
            b2m = b2["mass"] * mag
            b1["vx"] -= dx * b2m
            b1["vy"] -= dy * b2m
            b1["vz"] -= dz * b2m
            b2["vx"] += dx * b1m
            b2["vy"] += dy * b1m
            b2["vz"] += dz * b1m
        for body in bodies:
            body["x"] += dt * body["vx"]
            body["y"] += dt * body["vy"]
            body["z"] += dt * body["vz"]


def report_energy(bodies=bodies, pairs=pairs):
    e = 0.0
    for b1, b2 in pairs:
        dx = b1["x"] - b2["x"]
        dy = b1["y"] - b2["y"]
        dz = b1["z"] - b2["z"]
        e -= (b1["mass"] * b2["mass"]) / ((dx * dx + dy * dy + dz * dz) ** 0.5)
    for body in bodies:
        e += body["mass"] * (
            body["vx"] * body["vx"] + body["vy"] * body["vy"] + body["vz"] * body["vz"]
        ) / 2.0
    print("%.9f" % e)


def offset_momentum(ref, bodies=bodies):
    px = 0.0
    py = 0.0
    pz = 0.0
    for body in bodies:
        px -= body["vx"] * body["mass"]
        py -= body["vy"] * body["mass"]
        pz -= body["vz"] * body["mass"]
    ref["vx"] = px / ref["mass"]
    ref["vy"] = py / ref["mass"]
    ref["vz"] = pz / ref["mass"]

def main(n):
    offset_momentum(sun)
    report_energy()
    advance(0.01, n)
    report_energy()

main(int(sys.argv[1]))