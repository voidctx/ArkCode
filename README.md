# ArkCode

> **Reclaiming the lost art of systems engineering. A radical movement to liberate software from memory bloat, dependency addiction, and the culture of masking poor architecture with brute-force hardware.**

---

##  The Vision: Software That Respects the Planet & The Machine

We live in an era where software has lost its way. Simple softwares eat gigabytes of RAM, developer tools demand multi-thousand-dollar workstations, and software bloat is routinely brushed off with *"just buy more hardware"*. We grind LeetCode DSA puzzles for interviews, only to ship bloated frameworks that treat CPU cycles and system memory like infinite resources.

**ArkCode is created to spearhead a global shift in engineering philosophy.** 

We believe that:
1. **Efficiency is an ethical imperative.** Millions of capable machines—from low-power devices in developing regions to legacy computers—are rendered obsolete by software bloat. Efficient code democratizes access to technology.
2. **Hardware deserves respect.** A text editor and language server should never demand a 32GB RAM machine. Real engineering respects the physical boundaries of silicon.
3. **Zero-dependency engineering is the path to longevity.** By stripping away fragile dependency chains and framework churn, software can run for decades with rock-solid predictability and sub-megabyte memory footprints.

ArkCode is not just a light IDE; it is a proof-of-concept for the future of software development: **ultra-fast, hyper-efficient, self-contained cloud and local developer environments running everywhere.**

---

##  What It Does

* **Hyper-Efficient Footprint:** Operates on **0.6 MiB to 1.3 MiB of RAM** (averaging **~0.8 MiB**). Paired with a browser tab (~30 MiB), your entire IDE setup runs on practically zero resources.
* **Runs on Anything:** Works flawlessly on a **Raspberry Pi Zero (512 MiB RAM)**, single-board computers, edge servers, or low-cost cloud micro-VMs.
* **Full Developer Capabilities:** 
  * Workspace & file manager
  * Built-in code editor & multi-file search
  * Integrated terminal execution & build pipeline outputs
  * Activity logging & real-time updates
  * Language Server Protocol (LSP) integration via WebSockets
* **Anywhere Access:** Expose a single port to access your exact environment securely from any phone, tablet, or remote laptop.

---

##  How to Build & Run

### Prerequisites
* **Rust & Cargo**
* A C compiler (GCC/Clang or MSVC for terminal/process bindings)

### 1. Clone & Build
```bash
# Clone the repository
git clone https://github.com/your-repo/ArkCode.git
cd ArkCode

# Build optimized release binary
cargo build --release
```

### 2. Run the IDE Server
```bash
# Run on a specified custom port (e.g., 3000)
cargo run --release -- 3000
```
Alternatively, run the compiled binary directly:
```bash
./target/release/arkcode 8080
```

### 3. Open in Browser
Navigate to `http://localhost:8080` (or your configured port).

---

## 🛑 The Reality Check

* **Barebones Utilitarian UI:** The interface is built purely for low overhead and functionality. There are no heavy UI libraries or resource-draining CSS frameworks. 
* **Custom Engineering Trade-off:** Reducing external dependencies means writing low-level protocol handlers directly. This increases custom code volume slightly, but completely removes transitive vulnerability risks, library breakages, and dependency hell.

---

## 💻 Minimum Hardware Requirements

| Component | Minimum Requirement |
| :--- | :--- |
| **OS** | Linux / POSIX (or Windows for local execution) |
| **RAM** | **≥ 4 MiB free RAM** (backend needs ~0.8 MiB) |
| **CPU** | Any single-core ARM / x86 processor |
| **Network** | HTTP / WebSocket connection |

---

##  Addressing Counter-Arguments & Differing Perspectives

A movement that challenges prevailing industry habits inevitably sparks debate. Here is how ArkCode views common counter-arguments:

### 1. "Dependencies save time. Reinventing the wheel is bad engineering."
* **The Counter-Argument:** Relying on third-party libraries allows teams to ship features quickly without worrying about underlying protocol details.
* **Our Perspective:** Transitive dependency bloat creates security vulnerabilities, fragile build systems, and uncontrolled memory consumption. Taking ownership of core networking and buffer management ensures decades of software stability and zero-cost performance.

### 2. "Hardware is cheap. Sub-megabyte optimization is premature optimization."
* **The Counter-Argument:** Modern RAM is inexpensive, and developer time costs more than upgrading a computer.
* **Our Perspective:** Hardware abundance has bred wasteful software habits. When every background app consumes 1–2 GB of RAM, machines slow down needlessly. Sub-megabyte software unleashes cloud efficiency, reduces carbon footprint, and extends the lifespan of millions of computers worldwide.

### 3. "Rich visual design drives developer productivity."
* **The Counter-Argument:** Smooth animations, complex themes, and heavy UI components improve user experience.
* **Our Perspective:** Instant response time and zero latency are the ultimate UX features. ArkCode Web chooses utility and speed first, demonstrating that high performance *is* great design.

---

## 🚀 Future Vision

Future milestones will strip away remaining framework crates (`axum`/`tokio`) in favor of direct standard-library sockets and asynchronous event loops—lowering memory overhead even further while adding native multi-tenant authentication and TLS support.
