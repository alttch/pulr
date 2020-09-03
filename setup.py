__version__ = '0.1.8'

import setuptools

with open('README.md', 'r') as fh:
    long_description = fh.read()

setuptools.setup(
    name='pulr',
    version=__version__,
    author='Altertech',
    author_email='div@altertech.com',
    description='Industrial protocols data puller',
    long_description=long_description,
    long_description_content_type='text/markdown',
    url='https://github.com/alttch/pulr',
    packages=setuptools.find_packages(),
    license='Apache License 2.0',
    install_requires=['pyyaml', 'jsonschema', 'pytz', 'neotermcolor'],
    classifiers=(
        'Programming Language :: Python :: 3',
        'License :: OSI Approved :: Apache Software License',
        'Topic :: Communications',
        'Topic :: System :: Networking :: Monitoring :: Hardware Watchdog',
        'Topic :: Scientific/Engineering :: Human Machine Interfaces'),
    scripts=['bin/pulr'])
