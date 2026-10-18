"""Gym-style reinforcement-learning environment for harpoon-core.

The compiled extension is imported as ``harpoon_py``; this package adds the
Gymnasium-shaped :class:`HarpoonGymEnv` wrapper on top of it.
"""

from .env import HarpoonGymEnv

__all__ = ["HarpoonGymEnv"]
__version__ = "0.1.0"
