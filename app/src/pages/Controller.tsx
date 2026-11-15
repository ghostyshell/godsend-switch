import { useCallback, useEffect, useRef, useState } from "react";
import type { PageProps } from "../App";
import { api } from "../api";
import { Btn, Chip } from "../ui";

const FACE = [
  { id: "x", label: "X", className: "x" },
  { id: "y", label: "Y", className: "y" },
  { id: "a", label: "A", className: "a" },
  { id: "b", label: "B", className: "b" },
];

const SHOULDERS = ["l", "zl", "zr", "r"];

const DPAD = [
  { id: "up", label: "Up" },
  { id: "left", label: "Left" },
  { id: "down", label: "Down" },
  { id: "right", label: "Right" },
];

export default function Controller({ snap, toast }: PageProps) {
  const addr = snap.config.syscon_addr;
  const port = snap.config.syscon_port;
  const [held, setHeld] = useState<string[]>([]);
  const [left, setLeft] = useState<[number, number]>([0, 0]);
  const [right, setRight] = useState<[number, number]>([0, 0]);
  const heldRef = useRef<string[]>([]);

  const send = useCallback(
    async (buttons: string[], sticks?: { left?: [number, number]; right?: [number, number] }) => {
      try {
        await api.padSet(addr, port, buttons, sticks?.left ?? left, sticks?.right ?? right);
      } catch (err) {
        toast(`pad: ${String(err)}`);
      }
    },
    [addr, port, left, right, toast]
  );

  const toggle = (button: string) => {
    const next = held.includes(button) ? held.filter((b) => b !== button) : [...held, button];
    heldRef.current = next;
    setHeld(next);
    void send(next);
  };

  const tap = async (button: string) => {
    await send([button]);
    window.setTimeout(() => void send(heldRef.current), 90);
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const map: Record<string, string> = {
        ArrowUp: "up",
        ArrowDown: "down",
        ArrowLeft: "left",
        ArrowRight: "right",
        KeyZ: "b",
        KeyX: "a",
        KeyA: "y",
        KeyS: "x",
        KeyQ: "l",
        KeyE: "r",
        Enter: "plus",
        ShiftLeft: "minus",
      };
      const button = map[e.code];
      if (!button) return;
      e.preventDefault();
      if (e.repeat) return;
      void tap(button);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function detach() {
    setHeld([]);
    try {
      await api.padRelease(addr, port);
      toast("pad detached");
    } catch (err) {
      toast(`detach failed: ${String(err)}`);
    }
  }

  function updateStick(side: "left" | "right", axis: 0 | 1, value: number) {
    const next: [number, number] = side === "left" ? [...left] : [...right];
    next[axis] = value;
    if (side === "left") setLeft(next);
    else setRight(next);
    void send(held, side === "left" ? { left: next } : { right: next });
  }

  return (
    <div className="page">
      <div className="panel-head page-head">
        <h2>Virtual controller</h2>
        <div className="spacer" />
        <Chip tone={addr ? "good" : "bad"}>{addr ? `${addr}:${port}` : "sys-con address not set"}</Chip>
        <Btn small onClick={detach}>
          Detach pad
        </Btn>
      </div>

      <div className="panel switch-pad-panel">
        <div className="switch-pad">
          <div className="switch-pad-left">
            <div className="switch-shoulders">
              {SHOULDERS.slice(0, 2).map((id) => (
                <button
                  key={id}
                  className={`switch-shoulder ${held.includes(id) ? "on" : ""}`}
                  onPointerDown={() => toggle(id)}
                >
                  {id.toUpperCase()}
                </button>
              ))}
            </div>
            <div className="switch-dpad">
              {DPAD.map((button) => (
                <button
                  key={button.id}
                  className={`switch-dpad-btn ${button.id} ${held.includes(button.id) ? "on" : ""}`}
                  onPointerDown={() => toggle(button.id)}
                  aria-label={button.label}
                >
                  {button.label}
                </button>
              ))}
            </div>
          </div>

          <div className="switch-pad-middle">
            <button className={`pill ${held.includes("minus") ? "on" : ""}`} onClick={() => toggle("minus")}>
              -
            </button>
            <div className="switch-stick">
              <label>
                L x {left[0].toFixed(2)}
                <input
                  type="range"
                  min={-1}
                  max={1}
                  step={0.05}
                  value={left[0]}
                  onChange={(e) => updateStick("left", 0, Number(e.target.value))}
                />
              </label>
              <label>
                L y {left[1].toFixed(2)}
                <input
                  type="range"
                  min={-1}
                  max={1}
                  step={0.05}
                  value={left[1]}
                  onChange={(e) => updateStick("left", 1, Number(e.target.value))}
                />
              </label>
            </div>
            <button className={`pill ${held.includes("plus") ? "on" : ""}`} onClick={() => toggle("plus")}>
              +
            </button>
          </div>

          <div className="switch-pad-right">
            <div className="switch-shoulders">
              {SHOULDERS.slice(2).map((id) => (
                <button
                  key={id}
                  className={`switch-shoulder ${held.includes(id) ? "on" : ""}`}
                  onPointerDown={() => toggle(id)}
                >
                  {id.toUpperCase()}
                </button>
              ))}
            </div>
            <div className="switch-faces">
              {FACE.map((button) => (
                <button
                  key={button.id}
                  className={`switch-face ${button.className} ${held.includes(button.id) ? "on" : ""}`}
                  onPointerDown={() => toggle(button.id)}
                  aria-label={button.label}
                >
                  {button.label}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="switch-pad-secondary">
          <div className="switch-stick">
            <label>
              R x {right[0].toFixed(2)}
              <input
                type="range"
                min={-1}
                max={1}
                step={0.05}
                value={right[0]}
                onChange={(e) => updateStick("right", 0, Number(e.target.value))}
              />
            </label>
            <label>
              R y {right[1].toFixed(2)}
              <input
                type="range"
                min={-1}
                max={1}
                step={0.05}
                value={right[1]}
                onChange={(e) => updateStick("right", 1, Number(e.target.value))}
              />
            </label>
          </div>
          <div className="presets">
            {["home", "capture", "l", "r"].map((id) => (
              <Btn key={id} small onClick={() => tap(id)}>
                {id === "home" ? "Home" : id === "capture" ? "Capture" : id.toUpperCase()}
              </Btn>
            ))}
            <Btn
              small
              onClick={async () => {
                await send(["home"]);
                window.setTimeout(() => void send([]), 120);
              }}
            >
              Home tap
            </Btn>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-head">
          <h2>How this works</h2>
        </div>
        <p className="muted" style={{ padding: "0 14px 14px", lineHeight: 1.6 }}>
          The pad is sys-con's network controller: UDP packets to {port || 56789} on the console.
          Install sys-con, add <span className="mono">network_controller=1</span> to its config, and
          keep the PC and console on the same network. Buttons stay held until you click them again
          or press Detach. A game can ignore input while the HOME menu is open.
        </p>
        <div className="presets" style={{ padding: "0 14px 14px" }}>
          <Btn small onClick={() => tap("home")}>
            Send HOME
          </Btn>
          <Btn small onClick={() => send([])}>
            Release all
          </Btn>
        </div>
      </div>
    </div>
  );
}
