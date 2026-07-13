import React from "react";
import Image from "next/image";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./ui/dialog";
import { VisuallyHidden } from "./ui/visually-hidden";
import { About } from "./About";

interface LogoProps {
    isCollapsed: boolean;
}

const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(({ isCollapsed }, ref) => {
  return (
    <Dialog aria-describedby={undefined}>
      <DialogTrigger asChild>
        <button
          ref={ref}
          aria-label="About Briefli"
          className={`flex items-center border-none bg-transparent text-left transition-opacity hover:opacity-75 ${
            isCollapsed ? 'justify-center p-0' : 'gap-2.5'
          }`}
        >
          <Image src="/briefli-mark.svg" alt="" width={36} height={36} priority />
          {!isCollapsed && (
            <span>
              <span className="block font-brand text-[21px] font-semibold leading-5 text-briefli-ink">Briefli</span>
              <span className="mt-1 block text-[9px] font-semibold uppercase tracking-[0.14em] text-briefli-muted">
                Private meeting record
              </span>
            </span>
          )}
        </button>
      </DialogTrigger>
      <DialogContent>
        <VisuallyHidden>
          <DialogTitle>About Briefli</DialogTitle>
        </VisuallyHidden>
        <About />
      </DialogContent>
    </Dialog>
  );
});

Logo.displayName = "Logo";

export default Logo;