"""
This market maker replicates the binance OB at some granularity + some extensions for deep orders

- make use of binance L2 feed to determine binance depth profile
- high freq update of inside levels
- low freq update of outer levels
"""