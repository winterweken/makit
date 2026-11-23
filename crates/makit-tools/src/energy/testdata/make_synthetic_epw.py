"""Write synthetic.epw beside this script: a year of made-up hourly weather.

The energy engine's tests run on this file because a real weather file with a
confirmed licence was not available. The numbers are deterministic (no
randomness) and shaped like Calgary's: cold winters, summer afternoons above
the 24 C cooling set-point, the sun rising and setting, cloudy and clear days.
They are test data only: never model a building on them.

Run from anywhere with any Python 3: python3 make_synthetic_epw.py
"""
import math
from pathlib import Path

LAT, LON, TZ = 51.11, -114.02, -7.0
DAYS_IN_MONTH = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
# Field 4 of an EPW row: where each value came from. '?9' marks missing data.
SOURCES = '?9?9?9?9E0?9?9?9?9?9?9?9?9?9?9?9?9?9?9?9*9*9?9?9?9'

HEADER = [
    'LOCATION,SYNTHETIC CALGARY,AB,CAN,makit synthetic weather,000000,'
    '{},{},{},1084.1'.format(LAT, LON, TZ),
    'DESIGN CONDITIONS,0',
    'TYPICAL/EXTREME PERIODS,0',
    'GROUND TEMPERATURES,0',
    'HOLIDAYS/DAYLIGHT SAVINGS,No,0,0,0',
    'COMMENTS 1,Synthetic weather for makit\'s energy engine tests; not real data',
    'COMMENTS 2,Made by crates/makit-tools/src/energy/testdata/make_synthetic_epw.py',
    'DATA PERIODS,1,1,Data,Friday, 1/ 1,12/31',
]


def row(month, day, doy, h):
    temp = (4.5 - 16.0 * math.cos(2 * math.pi * (doy - 18) / 365)
            + 6.5 * math.cos(2 * math.pi * (h - 15) / 24))
    decl = math.radians(23.44 * math.sin(2 * math.pi * (284 + doy) / 365))
    ha = math.radians(15 * ((h - 0.5) + (LON - 15 * TZ) / 15 - 12))
    lat = math.radians(LAT)
    s = math.sin(lat) * math.sin(decl) + math.cos(lat) * math.cos(decl) * math.cos(ha)
    cloud = 0.35 + 0.65 * ((doy * 37) % 11) / 10
    if s > 0:
        dni = round(900 * math.exp(-0.14 / s) * cloud)
        dhi = round(60 + 110 * (1 - cloud) * s + 40 * s)
        ghi = round(dni * s + dhi)
    else:
        dni = dhi = ghi = 0
    fields = [2021, month, day, h, 60, SOURCES,
              '{:.1f}'.format(temp), '{:.1f}'.format(temp - 5.0), 70, 89000,
              9999, 9999, 9999, ghi, dni, dhi,
              999999, 999999, 999999, 9999,
              270, 4.0, 5, 5, 9999, 77777, 9, 999999999,
              999, 0.999, 999, 99, 999, 999, 99]
    return ','.join(str(f) for f in fields)


def main():
    lines = list(HEADER)
    doy = 0
    for month, days in enumerate(DAYS_IN_MONTH, start=1):
        for day in range(1, days + 1):
            doy += 1
            for h in range(1, 25):
                lines.append(row(month, day, doy, h))
    out = Path(__file__).resolve().parent / 'synthetic.epw'
    out.write_text('\n'.join(lines) + '\n', encoding='utf-8', newline='\n')


if __name__ == '__main__':
    main()
