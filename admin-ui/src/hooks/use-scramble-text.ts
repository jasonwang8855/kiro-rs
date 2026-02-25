import { useEffect, useRef, useState } from 'react'

const CHARS = '0123456789ABCDEFabcdef'

export function useScrambleText(
  target: string,
  enabled: boolean = true,
  duration: number = 400
): string {
  const [display, setDisplay] = useState(enabled ? randomize(target) : target)
  const frameRef = useRef<number>(0)

  useEffect(() => {
    if (!enabled || !target) {
      setDisplay(target)
      return
    }

    const start = performance.now()
    const tick = (now: number) => {
      const progress = Math.min((now - start) / duration, 1)
      const resolved = Math.floor(progress * target.length)
      const result = target.slice(0, resolved) + randomize(target.slice(resolved))
      setDisplay(result)

      if (progress < 1) {
        frameRef.current = requestAnimationFrame(tick)
      } else {
        setDisplay(target)
      }
    }

    frameRef.current = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frameRef.current)
  }, [target, enabled, duration])

  return display
}

function randomize(str: string): string {
  return str
    .split('')
    .map((c) => (c === ' ' ? ' ' : CHARS[Math.floor(Math.random() * CHARS.length)]))
    .join('')
}
