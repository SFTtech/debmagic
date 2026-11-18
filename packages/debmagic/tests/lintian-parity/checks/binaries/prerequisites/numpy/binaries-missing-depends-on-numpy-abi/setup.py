import setuptools
import numpy

setuptools.setup(
    ext_modules=[
        setuptools.Extension('basic', ['basic.c'],
            include_dirs=[numpy.get_include()]),
    ],
)
