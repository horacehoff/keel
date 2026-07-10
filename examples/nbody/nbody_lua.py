import sys
from math import sqrt


PI = 3.141592653589793
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


def advance(bodies, nbody, dt):
    for i in range(nbody):
        bi = bodies[i]
        bix, biy, biz, bimass = bi["x"], bi["y"], bi["z"], bi["mass"]
        bivx, bivy, bivz = bi["vx"], bi["vy"], bi["vz"]
        for j in range(i+1, nbody):
            bj = bodies[j]
            dx, dy, dz = bix-bj["x"], biy-bj["y"], biz-bj["z"]
            mag = sqrt(dx*dx + dy*dy + dz*dz)
            mag = dt / (mag * mag * mag)
            bm = bj["mass"]*mag
            bivx -= dx * bm
            bivy -= dy * bm
            bivz -= dz * bm
            bm = bimass*mag
            bj["vx"] += dx * bm
            bj["vy"] += dy * bm
            bj["vz"] += dz * bm
        bi["vx"] = bivx
        bi["vy"] = bivy
        bi["vz"] = bivz
        bi["x"] = bix + dt * bivx
        bi["y"] = biy + dt * bivy
        bi["z"] = biz + dt * bivz


def energy(bodies, nbody):
    e = 0.0
    for i in range(nbody):
        bi = bodies[i]
        vx, vy, vz, bim = bi["vx"], bi["vy"], bi["vz"], bi["mass"]
        e += 0.5 * bim * (vx*vx + vy*vy + vz*vz)
        for j in range(i+1, nbody):
            bj = bodies[j]
            dx, dy, dz = bi["x"]-bj["x"], bi["y"]-bj["y"], bi["z"]-bj["z"]
            distance = sqrt(dx*dx + dy*dy + dz*dz)
            e -= (bim * bj["mass"]) / distance
    return e


def offsetMomentum(b, nbody):
    px, py, pz = 0.0, 0.0, 0.0
    for i in range(nbody):
        bi = b[i]
        bim = bi["mass"]
        px += bi["vx"] * bim
        py += bi["vy"] * bim
        pz += bi["vz"] * bim
    b[0]["vx"] = -px / SOLAR_MASS
    b[0]["vy"] = -py / SOLAR_MASS
    b[0]["vz"] = -pz / SOLAR_MASS


N = int(sys.argv[1])
nbody = len(bodies)

offsetMomentum(bodies, nbody)
print("%.9f" % energy(bodies, nbody))
for i in range(1,N+1):
    advance(bodies, nbody, 0.01)
print("%.9f" % energy(bodies, nbody))
