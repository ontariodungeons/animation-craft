from setuptools import setup, find_namespace_packages

setup(
    name="magic",
    version="1.0.0",
    packages=find_namespace_packages(include=["cli_anything.*"]),
    package_data={
        "cli_anything.magic": ["skills/*.md"],
    },
    install_requires=[
        "click>=8.0.0",
        "mcp>=1.28,<2",
    ],
    entry_points={
        "console_scripts": [
            "magic=cli_anything.magic.gui:main",
            "magic-cli=cli_anything.magic.magic_cli:main",
        ],
    },
    python_requires=">=3.10",
)
