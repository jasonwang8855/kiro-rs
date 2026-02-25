import { Toaster as Sonner } from 'sonner'

type ToasterProps = React.ComponentProps<typeof Sonner>

const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      className="toaster group"
      toastOptions={{
        classNames: {
          toast:
            'group toast group-[.toaster]:rounded-lg group-[.toaster]:border group-[.toaster]:border-white/15 group-[.toaster]:bg-[#0A0A0A] group-[.toaster]:font-mono group-[.toaster]:text-white group-[.toaster]:shadow-2xl',
          description: 'group-[.toast]:text-neutral-400',
          actionButton:
            'group-[.toast]:bg-white group-[.toast]:text-black',
          cancelButton:
            'group-[.toast]:border group-[.toast]:border-white/15 group-[.toast]:bg-transparent group-[.toast]:text-neutral-300',
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
