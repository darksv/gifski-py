from setuptools import setup
from setuptools_rust import RustExtension, Binding

setup(
    name="gifski-py",
    version="0.1.0",
    packages=[],
    rust_extensions=[
        RustExtension('gifski', binding=Binding.PyO3)
    ],
    zip_safe=False,
)
