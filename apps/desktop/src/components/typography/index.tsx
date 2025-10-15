import { cn } from "@gitru/ui/lib/utils";
import React from "react";

const H1 = React.forwardRef<
	HTMLHeadingElement,
	React.HTMLAttributes<HTMLHeadingElement>
>((props, ref) => {
	return (
		<h1
			{...props}
			className={cn(
				"scroll-m-20 text-4xl font-extrabold tracking-tight lg:text-5xl",
				props.className,
			)}
			ref={ref}
		/>
	);
});
H1.displayName = "H1";

const H2 = React.forwardRef<
	HTMLHeadingElement,
	React.HTMLAttributes<HTMLHeadingElement>
>((props, ref) => {
	return (
		<h2
			{...props}
			className={cn(
				"scroll-m-20 border-b pb-2 text-3xl font-semibold tracking-tight first:mt-0",
				props.className,
			)}
			ref={ref}
		/>
	);
});
H2.displayName = "H2";

const H3 = React.forwardRef<
	HTMLHeadingElement,
	React.HTMLAttributes<HTMLHeadingElement>
>((props, ref) => {
	return (
		<h3
			{...props}
			className={cn(
				"scroll-m-20 text-2xl font-semibold tracking-tight",
				props.className,
			)}
			ref={ref}
		/>
	);
});
H3.displayName = "H3";

const H4 = React.forwardRef<
	HTMLHeadingElement,
	React.HTMLAttributes<HTMLHeadingElement>
>((props, ref) => {
	return (
		<h4
			{...props}
			className={cn(
				"scroll-m-20 text-xl font-semibold tracking-tight",
				props.className,
			)}
			ref={ref}
		/>
	);
});
H4.displayName = "H4";

const P = React.forwardRef<
	HTMLParagraphElement,
	React.HTMLAttributes<HTMLParagraphElement>
>((props, ref) => {
	return (
		<p
			{...props}
			className={cn("leading-7 [&:not(:first-child)]:mt-6", props.className)}
			ref={ref}
		/>
	);
});
P.displayName = "P";

const Blockquote = React.forwardRef<
	HTMLQuoteElement,
	React.HTMLAttributes<HTMLQuoteElement>
>((props, ref) => {
	return (
		<blockquote
			className={cn("mt-6 border-l-2 pl-6 italic", props.className)}
			ref={ref}
			{...props}
		/>
	);
});
Blockquote.displayName = "Blockquote";

const Table = React.forwardRef<
	HTMLTableElement,
	React.HTMLAttributes<HTMLTableElement>
>((props, ref) => {
	return <table className="w-full" ref={ref} {...props} />;
});
Table.displayName = "Table";

const Thead = React.forwardRef<
	HTMLTableSectionElement,
	React.HTMLAttributes<HTMLTableSectionElement>
>((props, ref) => {
	return (
		<thead className="m-0 border-t p-0 even:bg-muted" ref={ref} {...props} />
	);
});
Thead.displayName = "Thead";

const Tbody = React.forwardRef<
	HTMLTableSectionElement,
	React.HTMLAttributes<HTMLTableSectionElement>
>((props, ref) => {
	return (
		<tbody className="m-0 border-t p-0 even:bg-muted" ref={ref} {...props} />
	);
});
Tbody.displayName = "Tbody";

const Tr = React.forwardRef<
	HTMLTableRowElement,
	React.HTMLAttributes<HTMLTableRowElement>
>((props, ref) => {
	return <tr className="m-0 border-t p-0 even:bg-muted" ref={ref} {...props} />;
});
Tr.displayName = "Tr";

const Th = React.forwardRef<
	HTMLTableHeaderCellElement,
	React.HTMLAttributes<HTMLTableHeaderCellElement>
>((props, ref) => {
	return (
		<th
			className="border px-4 py-2 text-left font-bold [&[align=center]]:text-center [&[align=right]]:text-right"
			ref={ref}
			{...props}
		/>
	);
});
Th.displayName = "Th";

const Td = React.forwardRef<
	HTMLTableDataCellElement,
	React.HTMLAttributes<HTMLTableDataCellElement>
>((props, ref) => {
	return (
		<td
			className="border px-4 py-2 text-left [&[align=center]]:text-center [&[align=right]]:text-right"
			ref={ref}
			{...props}
		/>
	);
});
Td.displayName = "Td";

const Ul = React.forwardRef<
	HTMLUListElement,
	React.HTMLAttributes<HTMLUListElement>
>((props, ref) => {
	return (
		<ul className="my-6 ml-6 list-disc [&>li]:mt-2" ref={ref} {...props} />
	);
});
Ul.displayName = "Ul";

const Ol = React.forwardRef<
	HTMLOListElement,
	React.HTMLAttributes<HTMLOListElement>
>((props, ref) => {
	return (
		<ol className="my-6 ml-6 list-decimal [&>li]:mt-2" ref={ref} {...props} />
	);
});
Ol.displayName = "Ol";

const Li = React.forwardRef<HTMLLIElement, React.HTMLAttributes<HTMLLIElement>>(
	(props, ref) => {
		return <li className="m-0" ref={ref} {...props} />;
	},
);
Li.displayName = "Li";

const Code = React.forwardRef<HTMLElement, React.HTMLAttributes<HTMLElement>>(
	(props, ref) => {
		return (
			<code
				className="relative rounded bg-muted px-[0.3rem] py-[0.2rem] font-mono text-sm font-semibold"
				ref={ref}
				{...props}
			/>
		);
	},
);
Code.displayName = "Code";

const Lead = React.forwardRef<
	HTMLParagraphElement,
	React.HTMLAttributes<HTMLParagraphElement>
>((props, ref) => {
	return <p className="text-xl text-muted-foreground" ref={ref} {...props} />;
});
Lead.displayName = "Lead";

const Large = React.forwardRef<
	HTMLParagraphElement,
	React.HTMLAttributes<HTMLParagraphElement>
>((props, ref) => {
	return <p className="text-lg font-semibold" ref={ref} {...props} />;
});
Large.displayName = "Large";

const Small = React.forwardRef<
	HTMLParagraphElement,
	React.HTMLAttributes<HTMLParagraphElement>
>((props, ref) => {
	return (
		<p className="text-sm font-medium leading-none" ref={ref} {...props} />
	);
});
Small.displayName = "Small";

const Muted = React.forwardRef<
	HTMLParagraphElement,
	React.HTMLAttributes<HTMLParagraphElement>
>((props, ref) => {
	return (
		<p
			ref={ref}
			{...props}
			className={cn("text-sm text-muted-foreground", props.className)}
		/>
	);
});
Muted.displayName = "Muted";

export {
	H1,
	H2,
	H3,
	H4,
	P,
	Blockquote,
	Table,
	Thead,
	Tbody,
	Tr,
	Th,
	Td,
	Ul,
	Ol,
	Li,
	Code,
	Lead,
	Large,
	Small,
	Muted,
};
