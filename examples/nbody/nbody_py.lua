local function combinations(l)
    local result = {}
    for x = 1, #l - 1 do
        local ls = {unpack(l, x + 1, #l)}
        for _, y in ipairs(ls) do
            table.insert(result, { l[x], y })
        end
    end
    return result
end

local PI = 3.14159265358979323
local SOLAR_MASS = 4 * PI * PI
local DAYS_PER_YEAR = 365.24

local sun = { x = 0.0, y = 0.0, z = 0.0, vx = 0.0, vy = 0.0, vz = 0.0, mass = SOLAR_MASS }
local jupiter = {
    x = 4.84143144246472090,
    y = -1.16032004402742839,
    z = -0.103622044471123109,
    vx = 0.00166007664274403694 * DAYS_PER_YEAR,
    vy = 0.00769901118419740425 * DAYS_PER_YEAR,
    vz = -0.0000690460016974260023 * DAYS_PER_YEAR,
    mass = 0.000954791938424326609 * SOLAR_MASS,
}
local saturn = {
    x = 8.34336671824457987,
    y = 4.12479856412430479,
    z = -0.403523417114321381,
    vx = -0.00276742510726862411 * DAYS_PER_YEAR,
    vy = 0.00499852801234917238 * DAYS_PER_YEAR,
    vz = 0.0000230417297573763929 * DAYS_PER_YEAR,
    mass = 0.000285885980666130812 * SOLAR_MASS,
}
local uranus = {
    x = 12.8943695621391310,
    y = -15.1111514016986312,
    z = -0.223307578892655734,
    vx = 0.00296460137564761618 * DAYS_PER_YEAR,
    vy = 0.00237847173959480950 * DAYS_PER_YEAR,
    vz = -0.0000296589568540237556 * DAYS_PER_YEAR,
    mass = 0.0000436624404335156298 * SOLAR_MASS,
}
local neptune = {
    x = 15.3796971148509165,
    y = -25.9193146099879641,
    z = 0.179258772950371181,
    vx = 0.00268067772490389322 * DAYS_PER_YEAR,
    vy = 0.00162824170038242295 * DAYS_PER_YEAR,
    vz = -0.0000951592254519715870 * DAYS_PER_YEAR,
    mass = 0.0000515138902046611451 * SOLAR_MASS,
}

local bodies = { sun, jupiter, saturn, uranus, neptune }
local pairs = combinations(bodies)

function advance(dt, n, bodies, pairs)
    for _ = 1, n do
        for _, pair in ipairs(pairs) do
            local b1 = pair[1]
            local b2 = pair[2]
            local dx = b1.x - b2.x
            local dy = b1.y - b2.y
            local dz = b1.z - b2.z
            local mag = dt * ((dx * dx + dy * dy + dz * dz) ^ -1.5)
            local b1m = b1.mass * mag
            local b2m = b2.mass * mag
            b1.vx = b1.vx - dx * b2m
            b1.vy = b1.vy - dy * b2m
            b1.vz = b1.vz - dz * b2m
            b2.vx = b2.vx + dx * b1m
            b2.vy = b2.vy + dy * b1m
            b2.vz = b2.vz + dz * b1m
        end
        for _, body in ipairs(bodies) do
            body.x = body.x + dt * body.vx
            body.y = body.y + dt * body.vy
            body.z = body.z + dt * body.vz
        end
    end
end

function report_energy(bodies, pairs)
    local e = 0.0
    for _, pair in ipairs(pairs) do
        local b1 = pair[1]
        local b2 = pair[2]
        local dx = b1.x - b2.x
        local dy = b1.y - b2.y
        local dz = b1.z - b2.z
        e = e - (b1.mass * b2.mass) / ((dx * dx + dy * dy + dz * dz) ^ 0.5)
    end
    for _, body in ipairs(bodies) do
        e = e + body.mass * (body.vx * body.vx + body.vy * body.vy + body.vz * body.vz) / 2.0
    end
    print(string.format("%.9f", e))
end

function offset_momentum(ref, bodies)
    local px = 0.0
    local py = 0.0
    local pz = 0.0
    for _, body in ipairs(bodies) do
        px = px - body.vx * body.mass
        py = py - body.vy * body.mass
        pz = pz - body.vz * body.mass
    end
    ref.vx = px / ref.mass
    ref.vy = py / ref.mass
    ref.vz = pz / ref.mass
end

function main(n)
    offset_momentum(sun, bodies)
    report_energy(bodies, pairs)
    advance(0.01, n, bodies, pairs)
    report_energy(bodies, pairs)
end

main(tonumber(arg[1]))
