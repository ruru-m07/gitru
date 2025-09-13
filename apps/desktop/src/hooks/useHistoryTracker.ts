import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigationType } from "react-router-dom";

export function useHistoryTracker() {
	const [canGoBack, setCanGoBack] = useState(false);
	const [canGoForward, setCanGoForward] = useState(false);
	const historyStack = useRef<string[]>([]);
	const pointer = useRef<number>(-1);

	const location = useLocation();
	const navigationType = useNavigationType(); // PUSH / POP / REPLACE

	useEffect(() => {
		const pathname = location.pathname + location.search;

		if (navigationType === "PUSH") {
			historyStack.current = historyStack.current.slice(0, pointer.current + 1);
			historyStack.current.push(pathname);
			pointer.current++;
		} else if (navigationType === "POP") {
			const index = historyStack.current.indexOf(pathname);
			if (index !== -1) {
				pointer.current = index;
			}
		}

		setCanGoBack(pointer.current > 0);
		setCanGoForward(pointer.current < historyStack.current.length - 1);
	}, [location, navigationType]);

	return { canGoBack, canGoForward };
}
