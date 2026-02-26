export function SuccessCheck({ size = 48 }: { size?: number }) {
  return (
    <div className="relative flex items-center justify-center" style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox="0 0 24 24" fill="none" className="relative z-10">
        <path
          d="M5 13l4 4L19 7"
          stroke="white"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeDasharray="24"
          style={{ animation: 'draw-check 0.6s cubic-bezier(0.16,1,0.3,1) forwards' }}
        />
      </svg>
      <div
        className="absolute inset-0 rounded-full border border-white/30"
        style={{ animation: 'check-shockwave 0.6s cubic-bezier(0.16,1,0.3,1) forwards' }}
      />
    </div>
  )
}
