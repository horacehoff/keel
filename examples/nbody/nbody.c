#include <math.h>
#include <stdio.h>
#include <stdlib.h>

struct Body {
  double x;
  double y;
  double z;
  double vx;
  double vy;
  double vz;
  double mass;
};

void advance(struct Body *bodies, int nbody, double dt) {
  for (int i = 0; i < nbody; i++) {
    struct Body bi = bodies[i];
    double bix = bi.x;
    double biy = bi.y;
    double biz = bi.z;
    double bimass = bi.mass;
    double bivx = bi.vx;
    double bivy = bi.vy;
    double bivz = bi.vz;
    for (int j = i + 1; j < nbody; j++) {
      struct Body *bj = &bodies[j];
      double dx = bix - bj->x;
      double dy = biy - bj->y;
      double dz = biz - bj->z;
      double mag = sqrt(dx * dx + dy * dy + dz * dz);
      mag = dt / (mag * mag * mag);
      double bm = bj->mass * mag;
      bivx -= dx * bm;
      bivy -= dy * bm;
      bivz -= dz * bm;
      bm = bimass * mag;
      bj->vx += dx * bm;
      bj->vy += dy * bm;
      bj->vz += dz * bm;
    }
    bi.vx = bivx;
    bi.vy = bivy;
    bi.vz = bivz;
    bi.x = bix + dt * bivx;
    bi.y = biy + dt * bivy;
    bi.z = biz + dt * bivz;
    bodies[i] = bi;
  }
}

double energy(struct Body *bodies, int nbody) {
  double e = 0.0;
  for (int i = 0; i < nbody; i++) {
    struct Body bi = bodies[i];
    double vx = bi.vx;
    double vy = bi.vy;
    double vz = bi.vz;
    double bim = bi.mass;
    e += 0.5 * bim * (vx * vx + vy * vy + vz * vz);
    for (int j = i + 1; j < nbody; j++) {
      struct Body bj = bodies[j];
      double dx = bi.x - bj.x;
      double dy = bi.y - bj.y;
      double dz = bi.z - bj.z;
      double distance = sqrt(dx * dx + dy * dy + dz * dz);
      e -= (bim * bj.mass) / distance;
    }
  }
  return e;
}

void offsetMomentum(struct Body *bodies, int nbody, double solar_mass) {
  double px = 0.0;
  double py = 0.0;
  double pz = 0.0;
  for (int i = 0; i < nbody; i++) {
    struct Body bi = bodies[i];
    double bim = bi.mass;
    px += bi.vx * bim;
    py += bi.vy * bim;
    pz += bi.vz * bim;
  }
  bodies[0].vx = -px / solar_mass;
  bodies[0].vy = -py / solar_mass;
  bodies[0].vz = -pz / solar_mass;
}

int main(int argc, char *argv[]) {
  const double PI = 3.141592653589793;
  const double SOLAR_MASS = 4.0 * PI * PI;
  const double DAYS_PER_YEAR = 365.24;

  struct Body sun = {0.0, 0.0, 0.0, 0.0, 0.0, 0.0, SOLAR_MASS};

  struct Body jupiter = {4.84143144246472090,
                         -1.16032004402742839,
                         -0.103622044471123109,
                         0.00166007664274403694 * DAYS_PER_YEAR,
                         0.00769901118419740425 * DAYS_PER_YEAR,
                         -0.0000690460016974260023 * DAYS_PER_YEAR,
                         0.000954791938424326609 * SOLAR_MASS};

  struct Body saturn = {8.34336671824457987,
                        4.12479856412430479,
                        -0.403523417114321381,
                        -0.00276742510726862411 * DAYS_PER_YEAR,
                        0.00499852801234917238 * DAYS_PER_YEAR,
                        0.0000230417297573763929 * DAYS_PER_YEAR,
                        0.000285885980666130812 * SOLAR_MASS};

  struct Body uranus = {12.8943695621391310,
                        -15.1111514016986312,
                        -0.223307578892655734,
                        0.00296460137564761618 * DAYS_PER_YEAR,
                        0.00237847173959480950 * DAYS_PER_YEAR,
                        -0.0000296589568540237556 * DAYS_PER_YEAR,
                        0.0000436624404335156298 * SOLAR_MASS};

  struct Body neptune = {15.3796971148509165,
                         -25.9193146099879641,
                         0.179258772950371181,
                         0.00268067772490389322 * DAYS_PER_YEAR,
                         0.00162824170038242295 * DAYS_PER_YEAR,
                         -0.0000951592254519715870 * DAYS_PER_YEAR,
                         0.0000515138902046611451 * SOLAR_MASS};

  struct Body bodies[] = {sun, jupiter, saturn, uranus, neptune};
  int nbody = sizeof(bodies) / sizeof(bodies[0]);
  int N = atoi(argv[1]);

  offsetMomentum(bodies, nbody, SOLAR_MASS);
  printf("%f\n", energy(bodies, nbody));
  for (int i = 1; i < N + 1; i++) {
    advance(bodies, nbody, 0.01);
  }
  printf("%f\n", energy(bodies, nbody));
  return 0;
}