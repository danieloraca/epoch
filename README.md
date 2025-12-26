# epoch

A small, stable CLI to convert **unix epoch timestamps** or **formatted datetimes** into canonical time representations.

Supports:
- Unix timestamps (seconds or milliseconds)
- Formatted datetimes: `YYYY/MM/DD HH:MM:SS`
- RFC3339, unix, or JSON output

Designed to be **script-friendly**, **deterministic**, and easy to extend.

---

## Installation

### From source

```bash
git clone https://github.com/<your-username>/epoch.git
cd epoch
cargo build --release
