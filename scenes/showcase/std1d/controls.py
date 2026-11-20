"""Independent linear controls. Run with uv run python scenes/showcase/std1d/controls.py."""
import json
import math
import random


def average(values):
    return math.fsum(values) / len(values)


def main():
    rng = random.Random(312)
    cap = 0.0625 / (1 + math.sqrt(0.9375))
    omega = math.tau * cap
    assert math.isclose(omega, math.tau * (1 - math.sqrt(0.9375)))
    assert math.isclose(1 / (math.tau * 2), 1 / (4 * math.pi))
    misses_endpoint = 0
    for _ in range(4096):
        cosine = 1 - cap * rng.random()
        chord = math.sqrt(0.25 - 4 * (1 - cosine * cosine))
        near = 2 * cosine - chord
        shortened = near - 0.0002
        outside = (shortened - 2 * cosine) ** 2 - chord**2
        misses_endpoint += outside > 0
    assert misses_endpoint == 4096
    # An interior point reaches the far root. The shorter segment stays inside.
    exit_distance = math.sqrt(0.25 - 0.1**2)
    assert math.hypot(exit_distance - 0.0002, 0.1) < 0.5

    # Unit Lambertian receives r²/d² from a constant radiance sphere on axis.
    exact_direct = 0.5**2 / 2**2
    direct_errors = {}
    for spp in (16, 64, 256):
        means = []
        for _ in range(2048):
            # Draw from the spherical cap. cosine * omega / pi is f*cos/pdf.
            means.append(average([(1 - cap * rng.random()) * omega / math.pi for _ in range(spp)]))
        error = math.sqrt(average([(mean - exact_direct) ** 2 for mean in means]))
        direct_errors[spp] = error
    assert direct_errors[16] > 1.7 * direct_errors[64] > 1.7**2 * direct_errors[256]

    # Unit-Fresnel roughness-one furnace. This is the unclipped analytic GGX
    # integrand, using the exact directional albedo before any interpolation.
    directional_loss = math.log(2)
    average_energy = 4 / 3 * (1 - math.log(2))
    terms = []
    for i in range(1024):
        mu = math.sqrt((i + 0.5) / 1024)
        single = 0.5 / (1 + mu)
        multiple = directional_loss * (mu * math.log1p(1 / mu)) / (1 - average_energy)
        terms.append(0.5 * (single + multiple))
    furnace = average(terms)
    assert abs(furnace - 0.5) < 0.01

    def ink(light):
        return 0.3 if light < 0.6 else 0.9

    samples = [0.1, 0.2, 0.9, 1.0]
    ink_after = ink(average(samples))
    ink_before = average([ink(v) for v in samples])
    assert ink_after == 0.3 and ink_before == 0.6
    print(json.dumps({
        "sphere_solid_angle_sr": omega,
        "sphere_pdf_inverse_sr": 1 / omega,
        "outside_endpoint_segments": misses_endpoint,
        "direct_radiance": exact_direct,
        "direct_rms_by_spp": direct_errors,
        "rough_white_furnace_unclipped": furnace,
        "ink_after_linear_average": ink_after,
        "ink_before_linear_average": ink_before,
    }, indent=2))


if __name__ == "__main__":
    main()
