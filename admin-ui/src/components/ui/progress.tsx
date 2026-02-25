import * as React from 'react'
import { cn } from '@/lib/utils'

interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number
  max?: number
}

const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  ({ className, value = 0, max = 100, ...props }, ref) => {
    const percentage = Math.min(Math.max((value / max) * 100, 0), 100)

    return (
      <div
        ref={ref}
        className={cn(
          'relative h-1.5 w-full overflow-hidden rounded-full bg-white/10',
          className
        )}
        {...props}
      >
        <div
          className={cn(
            'h-full bg-gradient-to-r from-neutral-500 to-white transition-all duration-300 ease-[cubic-bezier(0.16,1,0.3,1)]'
          )}
          style={{ width: `${percentage}%` }}
        />
      </div>
    )
  }
)
Progress.displayName = 'Progress'

export { Progress }
