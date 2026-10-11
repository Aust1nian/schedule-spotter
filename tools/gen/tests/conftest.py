import pytest

from spotter_gen.gen import generate

SEEDS = range(0, 400)


@pytest.fixture(scope="session")
def cases():
    return [generate(s) for s in SEEDS]
