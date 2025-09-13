/* eslint-disable @typescript-eslint/no-non-null-asserted-optional-chain */
// "use client";

// import {
// 	Avatar,
// 	AvatarFallback,
// 	AvatarImage,
// } from "@noutify/ui/components/avatar";
// import { Button } from "@noutify/ui/components/button";
// import {
// 	DropdownMenu,
// 	DropdownMenuContent,
// 	DropdownMenuItem,
// 	DropdownMenuLabel,
// 	DropdownMenuSeparator,
// 	DropdownMenuTrigger,
// } from "@noutify/ui/components/dropdown-menu";
// import { Slider } from "@noutify/ui/components/slider";
// import { cn } from "@noutify/ui/lib/utils";
// import type { RestEndpointMethodTypes } from "@octokit/plugin-rest-endpoint-methods";
// import { LogOut } from "lucide-react";
// import type React from "react";
// import { type SessionType, useAuth } from "@/components/sidebar/avatarDropdown";
// import ThemeDropDown from "./themeDropDown";

// interface AvatarDropdownProps {
// 	user?: SessionType["user"];
// 	rateLimit:
// 		| RestEndpointMethodTypes["rateLimit"]["get"]["response"]["data"]
// 		| null;
// }

// const AvatarDropdown: React.FC<AvatarDropdownProps> = ({ user, rateLimit }) => {
// 	const { signOut } = useAuth();

// 	return (
// 		<DropdownMenu>
// 			<DropdownMenuTrigger asChild>
// 				<Button variant="ghost" className="h-auto p-0 hover:bg-transparent">
// 					<div className="relative">
// 						<Avatar className="size-8 border rounded-md">
// 							<AvatarImage
// 								src={user?.profile_avatar_url}
// 								alt={user?.profile_login}
// 							/>
// 							<AvatarFallback></AvatarFallback>
// 						</Avatar>
// 						{/* THE rate limit is betweeb o to 5000
//               if it between 0 to 3000 it will be green
//               if it between 3000 to 4000 it will be yellow
//               if it between 4000 to 5000 it will be red

//              */}
// 						<span
// 							className={cn(
// 								"border-background absolute -end-0.5 -bottom-0.5 size-3 rounded-full border-2",
// 								{
// 									// "bg-emerald-500": rateLimit?.rate?.used > 3000,
// 									// "bg-yellow-500":
// 									//   rateLimit?.rate?.used > 2000 &&
// 									//   rateLimit?.rate?.used <= 3000,
// 									// "bg-red-500": rateLimit?.rate?.used <= 2000,
// 									// if it;s used 60% till of the limit then show green. if it's used 80% then show yellow and if it's used 90% then show red
// 									"bg-emerald-500":
// 										rateLimit?.rate?.used! / rateLimit?.rate?.limit! <= 0.6,
// 									"bg-yellow-500":
// 										rateLimit?.rate?.used! / rateLimit?.rate?.limit! > 0.6 &&
// 										rateLimit?.rate?.used! / rateLimit?.rate?.limit! <= 0.8,
// 									"bg-red-500":
// 										rateLimit?.rate?.used! / rateLimit?.rate?.limit! > 0.8 &&
// 										rateLimit?.rate?.used! / rateLimit?.rate?.limit! <= 1,
// 								},
// 							)}
// 						></span>
// 					</div>
// 				</Button>
// 			</DropdownMenuTrigger>
// 			<DropdownMenuContent className="max-w-64 ml-2">
// 				<DropdownMenuLabel className="flex min-w-0 flex-col">
// 					<span className="truncate text-sm font-medium text-foreground">
// 						{user?.profile_name}
// 					</span>
// 					<span className="truncate text-xs font-normal text-muted-foreground">
// 						{user?.profile_login}
// 					</span>
// 				</DropdownMenuLabel>
// 				<DropdownMenuSeparator />
// 				<DropdownMenuLabel className="flex min-w-0 flex-col">
// 					<span className="truncate text-xs font-normal text-muted-foreground">
// 						Rate Limit ({rateLimit?.rate?.used}/{rateLimit?.rate?.limit})
// 					</span>
// 				</DropdownMenuLabel>
// 				<DropdownMenuItem>
// 					<Slider
// 						disabledNode
// 						defaultValue={[rateLimit?.rate?.used!]}
// 						min={0}
// 						max={rateLimit?.rate?.limit}
// 						disabled
// 						rangeStyle={{
// 							backgroundColor:
// 								rateLimit?.rate?.used! / rateLimit?.rate?.limit! <= 0.6
// 									? "#10b981"
// 									: rateLimit?.rate?.used! / rateLimit?.rate?.limit! > 0.6 &&
// 											rateLimit?.rate?.used! / rateLimit?.rate?.limit! <= 0.8
// 										? "#eab308"
// 										: "#ef4444",
// 						}}
// 						aria-label="rate limit"
// 					/>
// 				</DropdownMenuItem>
// 				<DropdownMenuLabel className="flex min-w-0 flex-col">
// 					<span className="truncate text-xs font-normal text-muted-foreground">
// 						reset in {(() => {
// 							const timeLeft = rateLimit?.rate?.reset! * 1000 - Date.now();
// 							const minutes = Math.floor(timeLeft / 60000);
// 							const seconds = Math.floor((timeLeft % 60000) / 1000);

// 							if (minutes > 0) {
// 								return `${minutes} min`;
// 							} else {
// 								return `${seconds} sec`;
// 							}
// 						})()}
// 					</span>
// 				</DropdownMenuLabel>

// 				<DropdownMenuSeparator />
// 				<DropdownMenuLabel>Preferences</DropdownMenuLabel>

// 				<ThemeDropDown />

// 				<DropdownMenuSeparator />
// 				<DropdownMenuItem
// 					onClick={() => signOut()}
// 					className="text-destructive focus:text-destructive focus:bg-destructive/20 mt-1.5"
// 				>
// 					<LogOut
// 						size={16}
// 						strokeWidth={2}
// 						className="_opacity-60"
// 						aria-hidden="true"
// 					/>
// 					<span>Logout</span>
// 				</DropdownMenuItem>
// 			</DropdownMenuContent>
// 		</DropdownMenu>
// 	);
// };

// export default AvatarDropdown;
