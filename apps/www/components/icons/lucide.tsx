/**
 * Animated icons from [lucide-animated](https://lucide-animated.com/).
 * Import from here instead of lucide-react.
 */
"use client";

export type { AnimatedIconHandle } from "./adapt-icon";
export { adaptAnimatedIcon, createSpinningIcon } from "./adapt-icon";

import { adaptAnimatedIcon, createSpinningIcon } from "./adapt-icon";
import { ArchiveIcon } from "./archive";
import { ArrowDownIcon } from "./arrow-down";
import { ArrowLeftIcon } from "./arrow-left";
import { ArrowRightIcon } from "./arrow-right";
import { ArrowUpIcon } from "./arrow-up";
import { ArrowUpRightIcon } from "./arrow-up-right";
import { BotIcon } from "./bot";
import { BriefcaseBusinessIcon } from "./briefcase-business";
import { CalendarCheck2Icon } from "./calendar-check-2";
import { CheckIcon } from "./check";
import { ChevronDownIcon } from "./chevron-down";
import { ChevronLeftIcon } from "./chevron-left";
import { ChevronRightIcon } from "./chevron-right";
import { ChevronUpIcon } from "./chevron-up";
import { ChevronsUpDownIcon } from "./chevrons-up-down";
import { CircleCheckIcon } from "./circle-check";
import { CircleHelpIcon } from "./circle-help";
import { DownloadIcon } from "./download";
import { ExternalLinkIcon } from "./external-link";
import { FileStackIcon } from "./file-stack";
import { FileTextIcon } from "./file-text";
import { GripHorizontalIcon } from "./grip-horizontal";
import { GripVerticalIcon } from "./grip-vertical";
import { LoaderCircleIcon } from "./loader-circle";
import { LogoutIcon } from "./logout";
import { MapPinOffIcon } from "./map-pin-off";
import { MenuIcon } from "./menu";
import { CirclePlus, CirclePlusIcon } from "./circle-plus-static";
import { MessageSquareIcon } from "./message-square";
import { MonitorCheckIcon } from "./monitor-check";
import { PanelRightOpenIcon } from "./panel-right-open";
import { PauseIcon } from "./pause";
import { PlayIcon } from "./play";
import { PlugZapIcon } from "./plug-zap";
import { Plus, PlusIcon } from "./plus-static";
import { RefreshCWIcon } from "./refresh-cw";
import { SearchIcon } from "./search";
import { SettingsIcon } from "./settings";
import { SparklesIcon } from "./sparkles";
import { Triangle } from "./triangle";
import { XIcon } from "./x";

const Archive = adaptAnimatedIcon(ArchiveIcon);
const ArrowDown = adaptAnimatedIcon(ArrowDownIcon);
const ArrowLeft = adaptAnimatedIcon(ArrowLeftIcon);
const ArrowRight = adaptAnimatedIcon(ArrowRightIcon);
const ArrowUp = adaptAnimatedIcon(ArrowUpIcon);
const ArrowUpRight = adaptAnimatedIcon(ArrowUpRightIcon);
const Bot = adaptAnimatedIcon(BotIcon);
const BriefcaseBusiness = adaptAnimatedIcon(BriefcaseBusinessIcon);
const CalendarClock = adaptAnimatedIcon(CalendarCheck2Icon);
const Check = adaptAnimatedIcon(CheckIcon);
const ChevronDown = adaptAnimatedIcon(ChevronDownIcon);
const ChevronLeft = adaptAnimatedIcon(ChevronLeftIcon);
const ChevronRight = adaptAnimatedIcon(ChevronRightIcon);
const ChevronUp = adaptAnimatedIcon(ChevronUpIcon);
const ChevronsUpDown = adaptAnimatedIcon(ChevronsUpDownIcon);
const CheckCircle2 = adaptAnimatedIcon(CircleCheckIcon);
const Download = adaptAnimatedIcon(DownloadIcon);
const ExternalLink = adaptAnimatedIcon(ExternalLinkIcon);
const FileText = adaptAnimatedIcon(FileTextIcon);
const Files = adaptAnimatedIcon(FileStackIcon);
const GripHorizontal = adaptAnimatedIcon(GripHorizontalIcon);
const GripVertical = adaptAnimatedIcon(GripVerticalIcon);
const Info = adaptAnimatedIcon(CircleHelpIcon);
const Loader2 = createSpinningIcon(LoaderCircleIcon);
const LogOut = adaptAnimatedIcon(LogoutIcon);
const Monitor = adaptAnimatedIcon(MonitorCheckIcon);
const MoreHorizontal = adaptAnimatedIcon(MenuIcon);
const PanelRight = adaptAnimatedIcon(PanelRightOpenIcon);
const Pause = adaptAnimatedIcon(PauseIcon);
const Play = adaptAnimatedIcon(PlayIcon);
const Plug = adaptAnimatedIcon(PlugZapIcon);
const RefreshCw = adaptAnimatedIcon(RefreshCWIcon);
const Search = adaptAnimatedIcon(SearchIcon);
const Settings2 = adaptAnimatedIcon(SettingsIcon);
const Sparkles = adaptAnimatedIcon(SparklesIcon);
const X = adaptAnimatedIcon(XIcon);
const MessageSquare = adaptAnimatedIcon(MessageSquareIcon);

const ArrowLeftToLine = ArrowLeft;
const ArrowRightToLine = ArrowRight;
const PinOff = adaptAnimatedIcon(MapPinOffIcon);
const Loader2Icon = Loader2;
const ArrowLeftToLineIcon = ArrowLeftToLine;
const ArrowRightToLineIcon = ArrowRightToLine;
const PinOffIcon = PinOff;
const Settings2Icon = Settings2;

export {
  Archive,
  ArchiveIcon,
  ArrowDown,
  ArrowDownIcon,
  ArrowLeft,
  ArrowLeftIcon,
  ArrowLeftToLine,
  ArrowLeftToLineIcon,
  ArrowRight,
  ArrowRightIcon,
  ArrowRightToLine,
  ArrowRightToLineIcon,
  ArrowUp,
  ArrowUpIcon,
  ArrowUpRight,
  ArrowUpRightIcon,
  Bot,
  BotIcon,
  BriefcaseBusiness,
  BriefcaseBusinessIcon,
  CalendarClock,
  Check,
  CheckCircle2,
  CheckIcon,
  ChevronDown,
  ChevronDownIcon,
  ChevronLeft,
  ChevronLeftIcon,
  ChevronRight,
  ChevronRightIcon,
  ChevronUp,
  ChevronUpIcon,
  ChevronsUpDown,
  ChevronsUpDownIcon,
  CirclePlus,
  CirclePlusIcon,
  Download,
  DownloadIcon,
  ExternalLink,
  ExternalLinkIcon,
  FileText,
  FileTextIcon,
  Files,
  GripHorizontal,
  GripHorizontalIcon,
  GripVertical,
  GripVerticalIcon,
  Info,
  Loader2,
  Loader2Icon,
  LoaderCircleIcon,
  LogOut,
  LogoutIcon,
  Monitor,
  MonitorCheckIcon,
  MoreHorizontal,
  PanelRight,
  PanelRightOpenIcon,
  Pause,
  PauseIcon,
  PinOff,
  PinOffIcon,
  Play,
  PlayIcon,
  Plug,
  PlugZapIcon,
  Plus,
  PlusIcon,
  RefreshCw,
  RefreshCWIcon,
  Search,
  SearchIcon,
  Settings2,
  SettingsIcon,
  Sparkles,
  SparklesIcon,
  Triangle,
  X,
  XIcon,
  MessageSquare,
  MessageSquareIcon,
  Settings2Icon,
};

